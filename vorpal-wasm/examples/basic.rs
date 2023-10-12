use vorpal_core::{ExternContext, Value, ExternInputId};
use vorpal_wasm::evaluate_node;

fn main() {
    //let node = vorpal_core::Node::Constant(Value::Scalar(5.0));
    let node = vorpal_core::Node::ExternInput(ExternInputId::new("Test".into()));

    dbg!(evaluate_node(&node, &ExternContext::default())).unwrap();
}
