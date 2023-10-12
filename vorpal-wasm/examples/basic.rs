use vorpal_core::{ExternContext, Value, ExternInputId, DataType};
use vorpal_wasm::evaluate_node;

fn main() {
    let test_input_name = ExternInputId::new("Test".into());

    let mut ctx = ExternContext::default();
    ctx.insert_input(&test_input_name, Value::Vec2([420.0, 69.0]));

    let node = vorpal_core::Node::ExternInput(test_input_name.clone(), DataType::Vec2);

    dbg!(evaluate_node(&node, &ctx)).unwrap();
}
