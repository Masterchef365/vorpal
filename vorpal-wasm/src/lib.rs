use anyhow::Result;
use std::fmt::Write;
use vorpal_core::*;
use wasm_bridge::*;

// TODO:
// Change Value to something like VectorValue<T, const N: usize>([T; N]);
// * Other datatypes
// * Longer vectors(?) - go by powers of two; octonions!

pub struct Engine {
    wasm_engine: wasm_bridge::Engine,
    cache: Option<CachedCompilation>,
}

struct CachedCompilation {
    node: Node,
    instance: Instance,
    store: Store<()>,
    mem: Memory,
}

impl Engine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            wasm_engine: wasm_bridge::Engine::new(&Default::default())?,
            cache: None,
        })
    }

    pub fn eval(&mut self, node: &Node, ctx: &ExternContext) -> Result<Value> {
        // Generate input list in random order
        let input_list = ctx
            .inputs()
            .iter()
            .map(|(name, value)| (name.clone(), value.dtype()))
            .collect::<Vec<(ExternInputId, DataType)>>();

        let mut store = Store::new(&self.wasm_engine, ());
        let (instance, analysis) = self.compile(node, input_list, false)?;
        self.exec_instance(&analysis, &instance, &mut store, ctx)
    }

    pub fn eval_image(&mut self, node: &Node, ctx: &ExternContext) -> Result<Vec<f32>> {
        // Assembly input list
        const RESOLUTION_KEY: &str = "Resolution (pixels)";
        const TIME_KEY: &str = "Time (seconds)";
        const POS_KEY: &str = "Position (pixels)";

        let res_key = &ExternInputId::new(RESOLUTION_KEY.into());
        let time_key = &ExternInputId::new(TIME_KEY.into());
        let pos_key = &ExternInputId::new(POS_KEY.into());

        let input_list = vec![
            // See vorpal-wasm-builtins' special_image_function
            (res_key.clone(), DataType::Vec2),
            (pos_key.clone(), DataType::Vec2),
            (time_key.clone(), DataType::Scalar),
        ];

        let Value::Vec2([width, height]) = ctx.inputs()[&res_key] else {
            panic!("Wrong vector type")
        };
        let Value::Scalar(time) = ctx.inputs()[&time_key] else {
            panic!("Wrong vector type")
        };
        let width = width as u32;
        let height = height as u32;

        let mut compile_data: CachedCompilation = self
            .cache
            .take()
            .filter(|cache| &cache.node == node)
            .map(|cache| Ok(cache))
            .unwrap_or_else(|| -> anyhow::Result<CachedCompilation> {
                let mut store = Store::new(&self.wasm_engine, ());
                let (kernel_module, _analysis) = self.compile(node, input_list, true)?;

                let mut linker = Linker::new(&mut self.wasm_engine);

                let memory_ty = MemoryType::new(100, None);
                let mem = Memory::new(&mut store, memory_ty)?;
                linker.define(&store, "env", "memory", mem)?;

                linker.module(&mut store, "builtins", &self.builtins_module()?)?;
                linker.module(&mut store, "kernel", &kernel_module)?;

                let instance = linker.instantiate(&mut store, &self.image_module()?)?;

                Ok(CachedCompilation {
                    node: node.clone(),
                    instance,
                    store,
                    mem,
                })
            })?;

        let func = compile_data
            .instance
            .get_typed_func::<(u32, u32, f32), u32>(&mut compile_data.store, "make_image")?;

        let ptr = func.call(&mut compile_data.store, (width, height, time))?;

        let mut out_image = vec![0_f32; (width * height * 4) as usize];
        compile_data.mem.read(
            &mut compile_data.store,
            ptr as usize,
            bytemuck::cast_slice_mut(&mut out_image),
        )?;

        self.cache = Some(compile_data);

        //dbg!(&out_image);

        Ok(out_image)
    }

    fn builtins_module(&self) -> Result<Module> {
        let builtins_wasm =
            include_bytes!("../../target/wasm32-unknown-unknown/release/vorpal_wasm_builtins.wasm");
        Ok(Module::new(&self.wasm_engine, builtins_wasm)?)
    }

    fn image_module(&self) -> Result<Module> {
        let builtins_wasm =
            include_bytes!("../../target/wasm32-unknown-unknown/release/vorpal_image.wasm");
        Ok(Module::new(&self.wasm_engine, builtins_wasm)?)
    }

    fn compile(
        &self,
        node: &Node,
        input_list: Vec<(ExternInputId, DataType)>,
        special: bool,
    ) -> Result<(Module, CodeAnalysis)> {
        let analysis = CodeAnalysis::new(Rc::new(node.clone()), input_list);
        let wat = analysis.compile_to_wat(special)?;
        let kernel_module = Module::new(&self.wasm_engine, wat)?;
        Ok((kernel_module, analysis))
    }

    fn exec_instance(
        &mut self,
        analysis: &CodeAnalysis,
        kernel_module: &Module,
        mut store: &mut Store<()>,
        ctx: &ExternContext,
    ) -> Result<Value> {
        let mut linker = Linker::new(&mut self.wasm_engine);
        linker.module(&mut store, "builtins", &self.builtins_module()?)?;
        let instance = linker.instantiate(&mut store, &kernel_module)?;

        let kernel = instance
            .get_func(&mut store, "kernel")
            .ok_or_else(|| anyhow::format_err!("Kernel function not found"))?;

        // Create parameter list
        let mut params = vec![];
        for (name, _dtype) in analysis.input_list() {
            let input_val = ctx.inputs()[name];
            params.extend(
                input_val
                    .iter_vector_floats()
                    .map(|f| Val::F32(f.to_bits())),
            );
        }

        // Create output list
        let mut results = vec![];
        let output_dtype = analysis.final_output_dtype();
        results.extend((0..output_dtype.n_lanes()).map(|_| Val::F32(0_f32.to_bits())));

        // Call the function
        kernel.call(&mut store, &params, &mut results)?;

        // Unwind the results
        Ok(match output_dtype {
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

/// Metadata for a node graph
struct CodeAnalysis {
    /// Mapping of a node to its corresponding local variable id
    locals: HashMap<HashRcByPtr<Node>, (LocalVarId, DataType)>,
    /// Mapping of an input name to its corresponding local variable id
    input_to_var: HashMap<ExternInputId, (LocalVarId, DataType)>,
    /// Next local variable ID to be produced
    next_var_id: LocalVarId,
    /// Root node
    root: HashRcByPtr<Node>,
    /// Ordered inputs; the function's parameters will match this order!
    input_list: Vec<(ExternInputId, DataType)>,
}

impl CodeAnalysis {
    fn new(node: Rc<Node>, input_list: Vec<(ExternInputId, DataType)>) -> Self {
        let root = HashRcByPtr(node);

        let mut instance = Self {
            next_var_id: 0,
            input_to_var: Default::default(),
            locals: Default::default(),
            input_list,
            root,
        };

        instance.find_inputs_and_locals_recursive(instance.root.clone());

        instance
    }

    pub fn final_output_dtype(&self) -> DataType {
        let (_, final_output_dtype) = self.locals[&self.root];
        final_output_dtype
    }

    pub fn input_list(&self) -> &[(ExternInputId, DataType)] {
        &self.input_list
    }

    pub fn compile_to_wat(&self, special: bool) -> Result<String> {
        // Build parameter list
        let mut input_var_ids = HashSet::new();
        let mut param_list_text = String::new();
        for (input_name, input_dtype) in &self.input_list {
            for lane in "xyzw".chars().take(input_dtype.n_lanes()) {
                if let Some((input_var_id, expected_dtype)) = self.input_to_var.get(input_name) {
                    assert_eq!(expected_dtype, input_dtype);
                    write!(&mut param_list_text, "(param ${input_var_id}_{lane} f32) ").unwrap();
                    input_var_ids.insert(input_var_id);
                } else {
                    // Dummy parameter
                    write!(&mut param_list_text, "(param f32) ").unwrap();
                }
            }
        }

        // Build result list
        let mut result_list_text = "(result ".to_string();
        for _ in 0..self.final_output_dtype().n_lanes() {
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

            for lane in "xyzw".chars().take(dtype.n_lanes()) {
                writeln!(&mut locals_text, "(local ${var_id}_{lane} f32) ").unwrap();
            }
        }

        // Compile instructions
        let mut function_body_text = String::new();
        self.compile_to_wat_recursive(&self.root, &mut function_body_text, &mut HashSet::new());

        // Build output stack
        let mut output_stack_text = String::new();
        let (var_id, _) = self.locals[&self.root];
        for lane in self.final_output_dtype().lane_names() {
            writeln!(&mut output_stack_text, "local.get ${var_id}_{lane}").unwrap();
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

        let special_image_function = if special {
            r#"
(func $special_image_function (param i32 f32 f32 f32 f32 f32)
;; Pull destination information onto the stack
    (local $x f32)
    (local $y f32)
    (local $z f32)
    (local $w f32)
;; These args are passed directly to the kernel
    local.get 1
    local.get 2
    local.get 3
    local.get 4
    local.get 5
    call $kernel
;; Pop kernel's implicit stack and store it in local variables
    local.set $w
    local.set $z
    local.set $y
    local.set $x
;; Store local variables on Rust's stack
    local.get 0
    local.get $w
    f32.store offset=12
    local.get 0
    local.get $z
    f32.store offset=8
    local.get 0
    local.get $y
    f32.store offset=4
    local.get 0
    local.get $x
    f32.store offset=0

)
(export "special_image_function" (func $special_image_function))
 "#
        } else {
            ""
        };

        let special_imports = if special {
            r#"(import "env" "memory" (memory (;0;) 17))"#
        } else {
            ""
        };

        let module_text = format!(
            r#"(module
;; Import memory if in special mode
{special_imports}
;; == External imports ==
{builtin_imports}

;; == Function declaration ==
  (func $kernel {param_list_text} {result_list_text}

;; Local variables
{locals_text}
;; == Compiled function (main program) ==
{function_body_text}
;; == Output stacking ==
{output_stack_text}
;; == Function end ==
  )
{special_image_function}
  (export "kernel" (func $kernel))
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

    fn find_inputs_and_locals_recursive(&mut self, node_hash: HashRcByPtr<Node>) -> DataType {
        if let Some((_number, dtype)) = self.locals.get(&node_hash) {
            return *dtype;
        }

        let new_id = self.gen_var_id();

        let dtype: DataType = match &*node_hash.0 {
            Node::ExternInput(name, dtype) => {
                self.input_to_var.insert(name.clone(), (new_id, *dtype));
                *dtype
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
                    writeln!(text, "f32.gt").unwrap();
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
