use std::rc::Rc;

use vorpal_core::{ComponentFn, ComponentInfixOp, DataType, ExternContext, ExternInputId, Value};
use vorpal_wasm::evaluate_node;

fn main() {
    let test_input_name = ExternInputId::new("Test".into());

    let mut ctx = ExternContext::default();
    ctx.insert_input(&test_input_name, Value::Vec2([420.0, 69.0]));

    /*
    let node = Rc::new(vorpal_core::Node::Make(
    vec![
    Rc::new(vorpal_core::Node::Constant(Value::Scalar(1.0))),
    Rc::new(vorpal_core::Node::Constant(Value::Scalar(1.0))),
    Rc::new(vorpal_core::Node::Constant(Value::Scalar(1.0))),
    Rc::new(vorpal_core::Node::Constant(Value::Scalar(1.0))),
    ],
    DataType::Vec4,
    ));
    */

    /*
    let node = vorpal_core::Node::Constant(Value::Scalar(3.0));

    let node = Rc::new(vorpal_core::Node::ComponentFn(
    vorpal_core::ComponentFn::Abs,
    Rc::new(node),
    ));
    */

    let a = vorpal_core::Node::Constant(Value::Scalar(2.0));
    let a = Rc::new(a);

    let b = vorpal_core::Node::Constant(Value::Scalar(-3.5));
    let b = Rc::new(b);

    for op in ComponentInfixOp::all() {
        println!("2 {} -3.5", op.symbol());
        let node = Rc::new(vorpal_core::Node::ComponentInfixOp(
            a.clone(),
            op,
            b.clone(),
        ));

        println!("{:?}", evaluate_node(&node, &ctx).unwrap());
    }

    for func in ComponentFn::all() {
        println!("{}(-3.5)", func.symbol());
        let node = Rc::new(vorpal_core::Node::ComponentFn(func, a.clone()));

        println!("{:?}", evaluate_node(&node, &ctx).unwrap());
    }
}
