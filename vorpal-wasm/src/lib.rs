use vorpal_core::*;
use anyhow::Result;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

pub fn evaluate_node(node: &Node, ctx: &ExternContext) -> Result<Value> {
    Engine::new()?.eval(&node)
}

pub struct Engine {
    wasm_engine: wasm_bridge::Engine,
}

impl Engine {
    pub fn new() -> Result<Self> {
        todo!()
    }

    pub fn eval(&mut self, node: &Node) -> Result<Value> {
        todo!()
    }
}

/// Compile a node into its equivalent
pub fn compile_to_wat(node: &Node) -> String {
    todo!()
}
