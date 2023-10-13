use anyhow::Result;
use std::fmt::Write;
use vorpal_core::*;
use wasm_bridge::*;

pub fn evaluate_node(node: &Node, ctx: &ExternContext) -> Result<Value> {
    Engine::new()?.eval(&node, ctx)
}

pub struct Engine {
    wasm_engine: wasm_bridge::Engine,
}

impl Engine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            wasm_engine: wasm_bridge::Engine::new(&Default::default())?,
        })
    }

    pub fn eval(&mut self, node: &Node, ctx: &ExternContext) -> Result<Value> {
        let mut codegen = CodeGenerator::new();

        let (wat, final_output_dtype) = codegen.compile_to_wat(node)?;
        let module = Module::new(&self.wasm_engine, wat)?;
        let mut store = Store::new(&self.wasm_engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;

        self.exec_instance(&codegen, &instance, &mut store, ctx, final_output_dtype)
    }

    fn exec_instance(
        &mut self,
        codegen: &CodeGenerator,
        instance: &Instance,
        mut store: &mut Store<()>,
        ctx: &ExternContext,
        final_output_dtype: DataType,
    ) -> Result<Value> {
        let kernel = instance
            .get_func(&mut store, "kernel")
            .ok_or_else(|| anyhow::format_err!("Kernel function not found"))?;

        // Create parameter list
        let mut params = vec![];
        for name in codegen.func_input_list.iter() {
            let input_val = ctx.inputs()[name];
            assert_eq!(codegen.inputs[name].1, input_val.dtype());
            params.extend(input_val.iter_vector_floats().map(|f| Val::F32(f.to_bits())));
        }

        // Create output list
        let mut results = vec![];
        results.extend((0..final_output_dtype.lanes()).map(|_| Val::F32(0_f32.to_bits())));

        // Call the function
        kernel.call(&mut store, &params, &mut results)?;

        // Unwind the results
        Ok(match final_output_dtype {
            DataType::Scalar => Value::Scalar(results[0].f32().unwrap()),
            DataType::Vec2 => {
                let mut val = [0.; 2];
                val.iter_mut()
                    .zip(&results)
                    .for_each(|(v, res)| *v = res.f32().unwrap());
                Value::Vec2(val)
            }
            DataType::Vec3 => {
                let mut val = [0.; 3];
                val.iter_mut()
                    .zip(&results)
                    .for_each(|(v, res)| *v = res.f32().unwrap());
                Value::Vec3(val)
            }
            DataType::Vec4 => {
                let mut val = [0.; 4];
                val.iter_mut()
                    .zip(&results)
                    .for_each(|(v, res)| *v = res.f32().unwrap());
                Value::Vec4(val)
            }
        })
    }
}

/// Denotes the "name" of a local variable; e.g. local.get 9
type LocalVarId = u32;

/// Compile a node into its equivalent
#[derive(Default)]
struct CodeGenerator {
    locals: HashMap<HashRcByPtr<Node>, (LocalVarId, DataType)>,
    inputs: HashMap<ExternInputId, (LocalVarId, DataType)>,
    next_var_id: LocalVarId,
    func_input_list: Vec<ExternInputId>,
}

impl CodeGenerator {
    pub fn new() -> Self {
        Self {
            next_var_id: 0,
            inputs: Default::default(),
            locals: Default::default(),
            func_input_list: vec![],
        }
    }

    pub fn compile_to_wat(&mut self, node: &Node) -> Result<(String, DataType)> {
        let node = Rc::new(node.clone());

        // Find input and output dtypes
        let final_output_dtype = self.find_inputs_and_locals_recursive(node.clone());

        // Build parameter list
        let mut input_var_ids = HashSet::new();
        let mut param_list_text = String::new();
        for (name, (var_id, dtype)) in &self.inputs {
            for lane in "xyzw".chars().take(dtype.lanes()) {
                write!(&mut param_list_text, "(param ${var_id}_{lane} f32) ").unwrap();
            }
            input_var_ids.insert(var_id);
            self.func_input_list.push(name.clone());
        }

        // Build result list
        let mut result_list_text = "(result ".to_string();
        for _ in 0..final_output_dtype.lanes() {
            result_list_text += "f32 ";
        }
        result_list_text += ")";

        // Build local list
        let mut locals_text = String::new();
        for (_node, (var_id, dtype)) in &self.locals {
            // Ignore inputs, which are already locals!
            if input_var_ids.contains(&var_id) {
                continue;
            }

            for lane in "xyzw".chars().take(dtype.lanes()) {
                writeln!(&mut locals_text, "(local ${var_id}_{lane} f32) ").unwrap();
            }
        }

        // Compile instructions
        let mut function_body_text = String::new();
        let hash_node = HashRcByPtr(node.clone());
        self.compile_to_wat_recursive(&hash_node, &mut function_body_text, &mut HashSet::new());

        // Build output stack
        let mut output_stack_text = String::new();
        let (var_id, _) = self.locals[&hash_node];
        for lane in final_output_dtype.lane_names() {
            writeln!(&mut output_stack_text, "local.get ${var_id}_{lane}").unwrap();
        }

        let module_text = format!(
            r#"(module
  (func $kernel {param_list_text} {result_list_text}
;; Local variables
{locals_text}
;; Compiled function
{function_body_text}
;; Output stacking
{output_stack_text}
  )
  (export "kernel" (func $kernel))
  (memory (;0;) 16)
  (export "memory" (memory 0))
)"#
        );

        println!("{}", module_text);

        Ok((module_text, final_output_dtype))
    }

    fn find_inputs_and_locals_recursive(&mut self, node: Rc<Node>) -> DataType {
        let node_hash = HashRcByPtr(node.clone());
        if let Some((_number, dtype)) = self.locals.get(&node_hash) {
            return *dtype;
        }

        let new_id = self.gen_var_id();

        let dtype: DataType = match &*node {
            Node::ExternInput(name, dtype) => {
                self.inputs.insert(name.clone(), (new_id, *dtype));
                *dtype
            }
            // Depth-first search
            Node::ComponentInfixOp(a, _, b) => {
                let a = self.find_inputs_and_locals_recursive(a.clone());
                let b = self.find_inputs_and_locals_recursive(b.clone());
                assert_eq!(a, b);
                a
            }
            Node::Dot(a, b) | Node::GetComponent(a, b) => {
                self.find_inputs_and_locals_recursive(a.clone());
                self.find_inputs_and_locals_recursive(b.clone());
                DataType::Scalar
            }
            Node::ExternSampler(_) => todo!(),
            Node::Constant(val) => val.dtype(),
            Node::Make(sub_nodes, _) => {
                for sub_node in sub_nodes {
                    assert_eq!(
                        self.find_inputs_and_locals_recursive(sub_node.clone()),
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
            Node::ComponentFn(_, a) => self.find_inputs_and_locals_recursive(a.clone()),
        };

        self.locals.insert(node_hash, (new_id, dtype));

        dtype
    }

    fn gen_var_id(&mut self) -> LocalVarId {
        let ret = self.next_var_id;
        self.next_var_id += 1;
        ret
    }

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
                assert_eq!(dtype.lanes(), sub_nodes.len());

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

                writeln!(text, ";; Get component ${out_var_id} = ${vector_id}[${index_id}]").unwrap();
                for lane in vector_dtype.lane_names().collect::<Vec<_>>().iter().rev() {
                    writeln!(text, "local.get ${vector_id}_{lane}").unwrap();
                }
                for i in 1..vector_dtype.lanes() {
                    // Check if the index equals this lane's index...
                    writeln!(text, "local.get ${index_id}_x ;;").unwrap();
                    writeln!(text, "f32.floor").unwrap();
                    writeln!(text, "f32.const {}.0", i).unwrap();
                    writeln!(text, "f32.gt").unwrap();
                    // Then set the output to this value
                    writeln!(text, "select").unwrap();
                }

                writeln!(text, "local.set ${out_var_id}_x").unwrap();

            }
            Node::ExternInput(_, _) => (),
            Node::Constant(value) => {
                writeln!(
                    text,
                    ";; Constant ${out_var_id} = {value:?}",
                )
                .unwrap();

                for (float, lane) in value.iter_vector_floats().zip(value.dtype().lane_names()) {
                    writeln!(text, "f32.const {float}").unwrap();
                    writeln!(text, "local.set ${out_var_id}_{lane}").unwrap();
                }
            },
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
                        _ => todo!("{}", infix),
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

                writeln!(
                    text,
                    ";; Dot product ${out_var_id} = ${a_id} * ${b_id}",
                )
                .unwrap();

                // Write code
                for (idx, lane) in a_dtype.lane_names().enumerate() {
                    writeln!(text, "local.get ${a_id}_{lane}").unwrap();
                    writeln!(text, "local.get ${b_id}_{lane}").unwrap();
                    writeln!(text, "f32.mul").unwrap();
                    if idx + 1 != out_dtype.lanes() {
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
                        _ => todo!("{}", func),
                    };

                    writeln!(text, "{}", op_text).unwrap();
                    writeln!(text, "local.set ${out_var_id}_{lane}").unwrap();
                }
            }
            _ => todo!("Node type {:?}", node.0)
        }

        writeln!(text).unwrap();
    }
}

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

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
