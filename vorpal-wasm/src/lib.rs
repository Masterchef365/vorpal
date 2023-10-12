use anyhow::Result;
use vorpal_core::*;
use wasm_bridge::*;
use std::fmt::Write;

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
        let mut codegen = CodeGenerator::new(ctx.inputs().iter().map(|(k, v)| (k.clone(), v.dtype())).collect());

        let wat = codegen.compile_to_wat(node)?;
        let module = Module::new(&self.wasm_engine, wat)?;
        let mut store = Store::new(&self.wasm_engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;

        self.exec_instance(&codegen, &instance, &mut store, ctx)
    }

    fn exec_instance(
        &mut self,
        codegen: &CodeGenerator,
        instance: &Instance,
        mut store: &mut Store<()>,
        ctx: &ExternContext,
    ) -> Result<Value> {
        let kernel = instance.get_typed_func::<(f32, f32), (f32, f32)>(&mut store, "kernel")?;
        //instance.get_func(&mut store, "kernel").unwrap();
        Ok(Value::Vec2(Vec2::from(
            kernel.call(&mut store, (2.5, 5.0))?,
        )))
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
    pub fn new(input_names: Vec<(ExternInputId, DataType)>) -> Self {
        // Parameter list
        let mut inputs = HashMap::new();
        let mut next_var_id = 0;
        for (name, dtype) in input_names {
            inputs.insert(name, (next_var_id, dtype));
            next_var_id += 1;
        }

        Self {
            next_var_id,
            inputs,
            locals: Default::default(),
            func_input_list: vec![],
        }
    }

    pub fn compile_to_wat(&mut self, node: &Node) -> Result<String> {
        // Find input and output dtypes
        let final_output_dtype = self.find_inputs_and_locals_recursive(Rc::new(node.clone()));

        // Build parameter list
        let mut param_list_text = String::new();
        for (name, (var_id, dtype)) in &self.inputs {
            for lane in "xyzw".chars().take(dtype.lanes()) {
                write!(&mut param_list_text, "(param ${var_id}_{lane} f32) ").unwrap();
            }
            self.func_input_list.push(name.clone());
        }

        // Build result list
        let mut result_list_text = "(result ".to_string();
        for _ in 0..final_output_dtype.lanes() {
            result_list_text += "f32 ";
        }
        result_list_text += ")";


        let function_body_text = "
    local.get 0
    local.get 1
    f32.sub
    local.get 0
    local.get 1
    f32.add
    ";

        let module_text = format!(
            r#"(module
  (func $kernel {param_list_text} {result_list_text}
{function_body_text}
  )
  (export "kernel" (func $kernel))
  (memory (;0;) 16)
  (export "memory" (memory 0))
)"#
        );

        println!("{}", module_text);

        Ok(module_text)
    }

    fn find_inputs_and_locals_recursive(&mut self, node: Rc<Node>) -> DataType {
        let node_hash = HashRcByPtr(node.clone());
        if let Some((_number, dtype)) = self.locals.get(&node_hash) {
            return *dtype;
        }

        let dtype: DataType = match &*node {
            Node::ExternInput(name, dtype) => {
                assert!(self.inputs.contains_key(&name));
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
                let a = self.find_inputs_and_locals_recursive(a.clone());
                let b = self.find_inputs_and_locals_recursive(b.clone());
                assert_eq!(a, b);
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

        let new_id = self.gen_var_id();
        self.locals.insert(node_hash, (new_id, dtype));

        dtype
    }

    fn gen_var_id(&mut self) -> LocalVarId {
        let ret = self.next_var_id;
        self.next_var_id += 1;
        ret
    }

    fn compile_to_wat_recursive(&mut self, node: &Node) -> Result<String> {
        todo!()
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
