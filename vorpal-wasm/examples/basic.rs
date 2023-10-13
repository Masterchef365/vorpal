use std::rc::Rc;

use vorpal_core::{ExternContext, Value, ExternInputId, DataType, ComponentInfixOp};
use vorpal_wasm::evaluate_node;

fn main() {
    let test_input_name = ExternInputId::new("Test".into());

    let mut ctx = ExternContext::default();
    ctx.insert_input(&test_input_name, Value::Vec2([420.0, 69.0]));

    let a = Rc::new(vorpal_core::Node::ExternInput(test_input_name.clone(), DataType::Vec2));
    let b = Rc::new(vorpal_core::Node::Constant(Value::Vec2([90.0, 10.0])));
    let node = Rc::new(vorpal_core::Node::ComponentInfixOp(a, ComponentInfixOp::Subtract, b));

    dbg!(evaluate_node(&node, &ctx)).unwrap();
}
