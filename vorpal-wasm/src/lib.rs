use vorpal_core::*;
use anyhow::Result;
use wasm_bridge::Linker;

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
    }
}

/// Compile a node into its equivalent
pub fn compile_to_wat(node: &Node) -> String {
}
