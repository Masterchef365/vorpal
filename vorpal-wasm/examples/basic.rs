use vorpal_core::{ExternContext, Value};
use vorpal_wasm::evaluate_node;

fn main() {
    dbg!(evaluate_node(&vorpal_core::Node::Constant(Value::Scalar(5.0)), &ExternContext::default()));
}