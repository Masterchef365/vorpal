use std::rc::Rc;

use vorpal_core::{ComponentInfixOp, DataType, ExternContext, ExternInputId, Value};
use vorpal_wasm::evaluate_node;

fn main() {
    let test_input_name = ExternInputId::new("Test".into());

    let mut ctx = ExternContext::default();
    ctx.insert_input(&test_input_name, Value::Vec2([420.0, 69.0]));

    let node = Rc::new(vorpal_core::Node::Make(
        vec![
            Rc::new(vorpal_core::Node::Constant(Value::Scalar(1.0))),
            Rc::new(vorpal_core::Node::Constant(Value::Scalar(1.0))),
            Rc::new(vorpal_core::Node::Constant(Value::Scalar(1.0))),
            Rc::new(vorpal_core::Node::Constant(Value::Scalar(1.0))),
        ],
        DataType::Vec4,
    ));

    dbg!(evaluate_node(&node, &ctx)).unwrap();
}
