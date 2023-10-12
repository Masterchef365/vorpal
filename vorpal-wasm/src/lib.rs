use anyhow::Result;
use vorpal_core::*;
use wasm_bridge::*;

pub fn evaluate_node(node: &Node, ctx: &ExternContext) -> Result<Value> {
    Engine::new()?.eval(&node, ctx.inputs())
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

    pub fn eval(&mut self, node: &Node, inputs: &HashMap<ExternInputId, Value>) -> Result<Value> {
        let input_names = inputs.keys().cloned().collect::<Vec<_>>();
        let mut codegen = CodeGenerator::new(input_names);

        let wat = codegen.compile_to_wat(node)?;
        let module = Module::new(&self.wasm_engine, wat)?;
        let mut store = Store::new(&self.wasm_engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;

        let kernel = instance.get_typed_func::<(f32, f32), (f32, f32)>(&mut store, "kernel")?;
        Ok(Value::Vec2(Vec2::from(kernel.call(&mut store, (2.5, 5.0))?)))
    }
}

/// Denotes the "name" of a local variable; e.g. local.get 9
type LocalVarId = u32;

/// Compile a node into its equivalent
#[derive(Default)]
struct CodeGenerator {
    params: HashMap<HashRcByPtr<Node>, LocalVarId>,
    inputs: HashMap<ExternInputId, LocalVarId>,
}

impl CodeGenerator {
    pub fn new(input_names: Vec<ExternInputId>) -> Self {
        let mut params = HashMap::new();
        let mut inputs = HashMap::new();

        Self { inputs, params }
    }

    pub fn compile_to_wat(&mut self, node: &Node) -> Result<String> {
        let param_list_text = "f32 f32";
        let result_list_text = "f32 f32";
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
  (func $kernel (param {param_list_text}) (result {result_list_text})
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

    pub fn compile_to_wat_recursive(&mut self, node: &Node) -> Result<String> {
        todo!()
    }
}

use std::collections::HashMap;
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
