use vorpal_core::{ExternContext, Value, ExternInputId, DataType};
use vorpal_wasm::evaluate_node;

fn main() {
    let test_input_name = ExternInputId::new("Test".into());

    let mut ctx = ExternContext::default();
    ctx.insert_input(&test_input_name, Value::Scalar(std::f32::consts::PI));

    let node = vorpal_core::Node::ExternInput(test_input_name.clone(), DataType::Scalar);

    dbg!(evaluate_node(&node, &ctx)).unwrap();
}
