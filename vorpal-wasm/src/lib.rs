use vorpal_core::*;
use anyhow::Result;
use wasm_bridge::*;

pub fn evaluate_node(node: &Node, ctx: &ExternContext) -> Result<Value> {
    Engine::new()?.eval(&node)
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

    pub fn eval(&mut self, node: &Node) -> Result<Value> {
        let wat = compile_to_wat(node)?;
        let module = Module::new(&self.wasm_engine, wat)?;
        let mut store = Store::new(&self.wasm_engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;

        let add = instance.get_typed_func::<(f32, f32), f32>(&mut store, "add")?;
        Ok(Value::Scalar(add.call(&mut store, (5.0, 5.0))?))
    }
}

/// Compile a node into its equivalent
pub fn compile_to_wat(node: &Node) -> Result<String> {
Ok(r#"
(module
  (type (;0;) (func (param f32 f32) (result f32)))
  (func $add (;0;) (type 0) (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.add
  )
  (memory (;0;) 16)
  (global $__stack_pointer (;0;) (mut i32) i32.const 1048576)
  (global (;1;) i32 i32.const 1048576)
  (global (;2;) i32 i32.const 1048576)
  (export "memory" (memory 0))
  (export "add" (func $add))
  (export "__data_end" (global 1))
  (export "__heap_base" (global 2))
  (@producers
    (language "Rust" "")
    (processed-by "rustc" "1.73.0 (cc66ad468 2023-10-03)")
  )
)"#.into())
}
