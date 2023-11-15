use anyhow::{ensure, Result};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use vorpal_core::*;

pub const BUILTINS_WASM: &[u8] =
    include_bytes!("../../target/wasm32-unknown-unknown/release/vorpal_wasm_builtins.wasm");

/// Denotes the "name" of a local variable; e.g. local.get 9
type LocalVarId = u32;

#[derive(Debug)]
pub enum InputParameter {
    ExternalVariable(ExternInputId, DataType),
    OutputPointer(LocalVarId),
}

/// Metadata for a node graph
pub struct CodeAnalysis {
    /// Mapping of a node to its corresponding local variable id
    locals: HashMap<HashRcByPtr<Node>, (LocalVarId, DataType)>,
    /// Mapping of an input name to its corresponding local variable id
    input_to_var: HashMap<ExternInputId, (LocalVarId, DataType)>,
    /// Next local variable ID to be produced
    next_var_id: LocalVarId,
    /// Root node
    root: HashRcByPtr<Node>,
    /// Ordered inputs; the function's parameters will match this order!
    input_list: Vec<InputParameter>,
}

impl CodeAnalysis {
    /// Inputs to the function will be arranged in the given order
    pub fn new(node: Rc<Node>, extern_inputs: &ParameterList) -> Self {
        let root = HashRcByPtr(node);

        let mut instance = Self {
            next_var_id: 0,
            input_to_var: Default::default(),
            locals: Default::default(),
            input_list: Default::default(),
            root,
        };

        // Add input pointer
        let input_ptr = instance.gen_var_id();
        instance
            .input_list
            .push(InputParameter::OutputPointer(input_ptr));

        // Add the rest of the parameters
        let mut sorted_inputs: Vec<InputParameter> = extern_inputs
            .0
            .iter()
            .map(|(id, ty)| InputParameter::ExternalVariable(id.clone(), *ty))
            .collect();
        sorted_inputs.sort_by_cached_key(|s| match s {
            InputParameter::OutputPointer(_) => unreachable!(),
            InputParameter::ExternalVariable(id, _) => id.clone(),
        });
        instance.input_list.extend(sorted_inputs);

        instance.find_inputs_and_locals_recursive(instance.root.clone());

        instance
    }

    /// Output datatype of the root node
    pub fn final_output_dtype(&self) -> DataType {
        let (_, final_output_dtype) = self.locals[&self.root];
        final_output_dtype
    }

    /// Get the input list passed to use at creation
    pub fn input_list(&self) -> &[InputParameter] {
        &self.input_list
    }

    pub fn func_name_rust(&self, func_name: &str) -> Result<String> {
        let mut param_list_text = String::new();


        writeln!(&mut param_list_text, r#"#[link(wasm_import_module = "{func_name}")]"#).unwrap();
        writeln!(&mut param_list_text, "{}", r#"extern "C" {"#).unwrap();
        writeln!(&mut param_list_text, "fn {func_name}(").unwrap();

        let space = "    ";

        for input_param in &self.input_list {
            match input_param {
                InputParameter::OutputPointer(_) => {
                    // Pointer for output float data (*mut f32)
                    writeln!(&mut param_list_text, "{space}out_ptr: *mut f32, ").unwrap();
                }
                InputParameter::ExternalVariable(input_name, input_dtype) => {
                    let nicer_input_name: String = input_name
                        .to_string()
                        .to_lowercase()
                        .chars()
                        .filter_map(|c| match c {
                            c if c.is_alphanumeric() => Some(c),
                            c if c.is_whitespace() => Some('_'),
                            _ => None,
                        })
                        .collect();

                    if input_dtype.n_lanes() == 1 {
                        writeln!(&mut param_list_text, "{space}{nicer_input_name}: f32, ").unwrap();
                    } else {
                        for lane in "xyzw".chars().take(input_dtype.n_lanes()) {
                            writeln!(&mut param_list_text, "{space}{nicer_input_name}_{lane}: f32, ").unwrap();
                        }
                    }
                }
            }
        }

        writeln!(&mut param_list_text, ");").unwrap();
        writeln!(&mut param_list_text, "{}", "}").unwrap();

        Ok(param_list_text)
    }

    pub fn func_name_wat(&self, func_name: &str) -> Result<String> {
        let mut param_list_text = String::new();

        write!(&mut param_list_text, "(func ${func_name} ").unwrap();

        for input_param in &self.input_list {
            match input_param {
                InputParameter::OutputPointer(input_var_id) => {
                    // Pointer for output float data (*mut f32)
                    write!(&mut param_list_text, "(param ${input_var_id} i32) ").unwrap();
                }
                InputParameter::ExternalVariable(input_name, input_dtype) => {
                    for lane in "xyzw".chars().take(input_dtype.n_lanes()) {
                        if let Some((input_var_id, expected_dtype)) =
                            self.input_to_var.get(input_name)
                        {
                            // External input parameter
                            ensure!(
                                expected_dtype == input_dtype,
                                "Datatype mismatch; expected {} got {}",
                                expected_dtype,
                                input_dtype
                            );
                            write!(&mut param_list_text, "(param ${input_var_id}_{lane} f32) ")
                                .unwrap();
                        } else {
                            // Dummy parameter to keep the ordering of the inputs
                            write!(&mut param_list_text, "(param f32) ").unwrap();
                        }
                    }
                }
            }
        }

        Ok(param_list_text)
    }

    /// Compile this analysis to webassembly
    pub fn compile_to_wat(&self, func_name: &str) -> Result<String> {
        // Build parameter list
        let mut input_var_ids = HashSet::new();
        for input_param in &self.input_list {
            match input_param {
                InputParameter::ExternalVariable(input_name, input_dtype) => {
                    for _ in 0..input_dtype.n_lanes() {
                        if let Some((input_var_id, _)) = self.input_to_var.get(input_name) {
                            input_var_ids.insert(input_var_id);
                        }
                    }
                }
                _ => (),
            }
        }

        // Build local list
        let mut locals_text = String::new();
        for (_node, (var_id, dtype)) in &self.locals {
            // Ignore inputs, which are already locals!
            if input_var_ids.contains(&var_id) {
                continue;
            }

            for lane in "xyzw".chars().take(dtype.n_lanes()) {
                writeln!(&mut locals_text, "(local ${var_id}_{lane} f32) ").unwrap();
            }
        }

        let func_decl = self.func_name_wat(func_name)?;

        // Compile instructions
        let mut function_body_text = String::new();
        self.compile_to_wat_recursive(&self.root, &mut function_body_text, &mut HashSet::new());

        // Write to output pointer
        let mut output_stack_text = String::new();
        let (var_id, _) = self.locals[&self.root];
        let InputParameter::OutputPointer(output_ptr_id) = self.input_list[0] else {
            unreachable!()
        };
        for (idx, lane) in self.final_output_dtype().lane_names().enumerate() {
            let offset = idx * 4; // f32 is 4 bytes
            writeln!(&mut output_stack_text, "local.get ${output_ptr_id}").unwrap();
            writeln!(&mut output_stack_text, "local.get ${var_id}_{lane}").unwrap();
            writeln!(&mut output_stack_text, "f32.store offset={offset}").unwrap();
        }

        let builtin_imports = r#"(import "builtins" "sine" (func $builtin_sine (param f32) (result f32)))
(import "builtins" "cosine" (func $builtin_cosine (param f32) (result f32)))
(import "builtins" "tangent" (func $builtin_tangent (param f32) (result f32)))
(import "builtins" "natural_log" (func $builtin_natural_log (param f32) (result f32)))
(import "builtins" "natural_exp" (func $builtin_natural_exp (param f32) (result f32)))

(import "builtins" "power" (func $builtin_power (param f32 f32) (result f32)))
(import "builtins" "logbase" (func $builtin_logbase (param f32 f32) (result f32)))
(import "builtins" "greater_than" (func $builtin_greater_than (param f32 f32) (result f32)))
(import "builtins" "less_than" (func $builtin_less_than (param f32 f32) (result f32)))"#;

        let module_text = format!(
            r#"(module
;; Import memory
(import "env" "memory" (memory (;0;) 17))
;; == External imports ==
{builtin_imports}

;; == Function declaration ==
{func_decl}

;; Local variables
{locals_text}
;; == Compiled function (main program) ==
{function_body_text}
;; == Output stacking ==
{output_stack_text}
;; == Function end ==
  )
  (export "{func_name}" (func ${func_name}))
)"#
        );

        /*
        let lined_text: String = module_text
        .lines()
        .enumerate()
        .map(|(idx, line)| format!("{:>4}: {:}\n", idx + 1, line))
        .collect();
        eprintln!("{}", lined_text);
        */

        Ok(module_text)
    }

    /// A first pass which finds all local variables and inputs which are used
    fn find_inputs_and_locals_recursive(&mut self, node_hash: HashRcByPtr<Node>) -> DataType {
        if let Some((_number, dtype)) = self.locals.get(&node_hash) {
            return *dtype;
        }

        // NOTE: May or may not be used
        let new_id = self.gen_var_id();

        let dtype: DataType = match &*node_hash.0 {
            Node::ExternInput(name, dtype) => {
                if let Some((existing_id, _)) = self.input_to_var.get(&name) {
                    // This input already exists. Stop before generating a new one!
                    self.locals
                        .insert(node_hash.clone(), (*existing_id, *dtype));
                    return *dtype;
                } else {
                    self.input_to_var.insert(name.clone(), (new_id, *dtype));
                    *dtype
                }
            }
            // Depth-first search
            Node::ComponentInfixOp(a, _, b) => {
                let a = self.find_inputs_and_locals_recursive(HashRcByPtr(a.clone()));
                let b = self.find_inputs_and_locals_recursive(HashRcByPtr(b.clone()));
                assert_eq!(a, b);
                a
            }
            Node::Dot(a, b) | Node::GetComponent(a, b) => {
                self.find_inputs_and_locals_recursive(HashRcByPtr(a.clone()));
                self.find_inputs_and_locals_recursive(HashRcByPtr(b.clone()));
                DataType::Scalar
            }
            Node::ExternSampler(_) => todo!(),
            Node::Constant(val) => val.dtype(),
            Node::Make(sub_nodes, _) => {
                for sub_node in sub_nodes {
                    assert_eq!(
                        self.find_inputs_and_locals_recursive(HashRcByPtr(sub_node.clone())),
                        DataType::Scalar
                    );
                }
                match sub_nodes.len() {
                    1 => DataType::Scalar,
                    2 => DataType::Vec2,
                    3 => DataType::Vec3,
                    4 => DataType::Vec4,
                    other => panic!("Attempted to make an vector type; {}", other),
                }
            }
            Node::ComponentFn(_, a) => {
                self.find_inputs_and_locals_recursive(HashRcByPtr(a.clone()))
            }
        };

        self.locals.insert(node_hash, (new_id, dtype));

        dtype
    }

    /// Generate a new local variable ID
    fn gen_var_id(&mut self) -> LocalVarId {
        let ret = self.next_var_id;
        self.next_var_id += 1;
        ret
    }

    // Explore the graph left-hand-side-first, so that inputs are computed before outputs
    // Meanwhile assemble functions as we go
    fn compile_to_wat_recursive(
        &self,
        node: &HashRcByPtr<Node>,
        text: &mut String,
        visited: &mut HashSet<HashRcByPtr<Node>>,
    ) {
        if !visited.insert(node.clone()) {
            return;
        }

        let (out_var_id, out_dtype) = self.locals[node];

        match &*node.0 {
            // Don't need to do anything, input is already provided for us
            Node::Make(sub_nodes, dtype) => {
                assert_eq!(dtype.n_lanes(), sub_nodes.len());

                for sub_node in sub_nodes {
                    let sub_node = HashRcByPtr(sub_node.clone());
                    self.compile_to_wat_recursive(&sub_node, text, visited);
                }

                writeln!(text, ";; Make vector ${out_var_id}").unwrap();
                for (lane, sub_node) in out_dtype.lane_names().zip(sub_nodes) {
                    let sub_node = HashRcByPtr(sub_node.clone());
                    let (a_id, dtype) = self.locals[&sub_node];
                    assert_eq!(dtype, DataType::Scalar);
                    writeln!(text, "local.get ${a_id}_x").unwrap();
                    writeln!(text, "local.set ${out_var_id}_{lane}").unwrap();
                }
            }
            Node::GetComponent(vector_node, index_node) => {
                for sub_node in [vector_node, index_node] {
                    let sub_node = HashRcByPtr(sub_node.clone());
                    self.compile_to_wat_recursive(&sub_node, text, visited);
                }

                let (vector_id, vector_dtype) = self.locals[&HashRcByPtr(vector_node.clone())];
                let (index_id, index_dtype) = self.locals[&HashRcByPtr(index_node.clone())];
                assert_eq!(index_dtype, DataType::Scalar);

                writeln!(
                    text,
                    ";; Get component ${out_var_id} = ${vector_id}[${index_id}]"
                )
                .unwrap();
                for lane in vector_dtype.lane_names().collect::<Vec<_>>().iter().rev() {
                    writeln!(text, "local.get ${vector_id}_{lane}").unwrap();
                }
                for i in 1..vector_dtype.n_lanes() {
                    // Check if the index equals this lane's index...
                    writeln!(text, "local.get ${index_id}_x ;;").unwrap();
                    writeln!(text, "f32.floor").unwrap();
                    writeln!(text, "f32.const {}.0", i).unwrap();
                    writeln!(text, "f32.ge").unwrap();
                    // Then set the output to this value
                    writeln!(text, "select").unwrap();
                }

                writeln!(text, "local.set ${out_var_id}_x").unwrap();
            }
            Node::ExternInput(_, _) => (),
            Node::Constant(value) => {
                writeln!(text, ";; Constant ${out_var_id} = {value:?}",).unwrap();

                for (float, lane) in value.iter_vector_floats().zip(value.dtype().lane_names()) {
                    writeln!(text, "f32.const {float}").unwrap();
                    writeln!(text, "local.set ${out_var_id}_{lane}").unwrap();
                }
            }
            Node::ComponentInfixOp(a, infix, b) => {
                // Visit child nodes first
                let a = HashRcByPtr(a.clone());
                let b = HashRcByPtr(b.clone());
                self.compile_to_wat_recursive(&a, text, visited);
                self.compile_to_wat_recursive(&b, text, visited);

                // Write comment
                let (a_id, _) = self.locals[&a];
                let (b_id, _) = self.locals[&b];
                writeln!(
                    text,
                    ";; Component infix op ${out_var_id} = ${a_id} {} ${b_id}",
                    infix.symbol()
                )
                .unwrap();

                // Write code
                for lane in out_dtype.lane_names() {
                    writeln!(text, "local.get ${a_id}_{lane}").unwrap();
                    writeln!(text, "local.get ${b_id}_{lane}").unwrap();
                    let op_text = match infix {
                        ComponentInfixOp::Add => "f32.add",
                        ComponentInfixOp::Subtract => "f32.sub",
                        ComponentInfixOp::Divide => "f32.div",
                        ComponentInfixOp::Multiply => "f32.mul",
                        ComponentInfixOp::Power => "call $builtin_power",
                        ComponentInfixOp::Logbase => "call $builtin_logbase",
                        ComponentInfixOp::GreaterThan => "call $builtin_greater_than",
                        ComponentInfixOp::LessThan => "call $builtin_less_than",
                    };

                    writeln!(text, "{}", op_text).unwrap();
                    writeln!(text, "local.set ${out_var_id}_{lane}").unwrap();
                }
            }
            Node::Dot(a, b) => {
                // Visit child nodes first
                let a = HashRcByPtr(a.clone());
                let b = HashRcByPtr(b.clone());
                self.compile_to_wat_recursive(&a, text, visited);
                self.compile_to_wat_recursive(&b, text, visited);

                // Write comment
                let (a_id, a_dtype) = self.locals[&a];
                let (b_id, b_dtype) = self.locals[&b];

                assert_eq!(a_dtype, b_dtype);

                writeln!(text, ";; Dot product ${out_var_id} = ${a_id} * ${b_id}",).unwrap();

                // Write code
                for (idx, lane) in a_dtype.lane_names().enumerate() {
                    writeln!(text, "local.get ${a_id}_{lane}").unwrap();
                    writeln!(text, "local.get ${b_id}_{lane}").unwrap();
                    writeln!(text, "f32.mul").unwrap();
                    if idx + 1 != out_dtype.n_lanes() {
                        writeln!(text, "f32.add").unwrap();
                    }
                }
                writeln!(text, "local.set ${out_var_id}_x").unwrap();
            }
            Node::ComponentFn(func, a) => {
                // Visit child nodes first
                let a = HashRcByPtr(a.clone());
                self.compile_to_wat_recursive(&a, text, visited);

                // Write comment
                let (a_id, _) = self.locals[&a];
                writeln!(
                    text,
                    ";; Component function ${out_var_id} = {}(${a_id})",
                    func.symbol(),
                )
                .unwrap();

                // Write code
                for lane in out_dtype.lane_names() {
                    writeln!(text, "local.get ${a_id}_{lane}").unwrap();
                    let op_text = match func {
                        ComponentFn::Ceil => "f32.ceil",
                        ComponentFn::Floor => "f32.floor",
                        ComponentFn::Abs => "f32.abs",
                        ComponentFn::Sine => "call $builtin_sine",
                        ComponentFn::Cosine => "call $builtin_cosine",
                        ComponentFn::Tangent => "call $builtin_tangent",
                        ComponentFn::NaturalLog => "call $builtin_natural_log",
                        ComponentFn::NaturalExp => "call $builtin_natural_exp",
                    };

                    writeln!(text, "{}", op_text).unwrap();
                    writeln!(text, "local.set ${out_var_id}_{lane}").unwrap();
                }
            }
            _ => todo!("Node type {:?}", node.0),
        }

        writeln!(text).unwrap();
    }
}

/// Instead of hashing by the _contents_ of an Rc smart pointer,
/// we are hashing by its pointer. This makes it such that we can store
/// a hashmap containing nodes in a graph.
#[derive(Clone, Default)]
struct HashRcByPtr<T>(pub Rc<T>);

impl<T> Hash for HashRcByPtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state)
    }
}

impl<T> Eq for HashRcByPtr<T> {}

impl<T> PartialEq for HashRcByPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::as_ptr(&self.0).eq(&Rc::as_ptr(&other.0))
    }
}
