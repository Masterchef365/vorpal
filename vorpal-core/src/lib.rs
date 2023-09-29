use std::rc::Rc;

type Scalar = f32;
type Vec2 = [f32; 2];

#[derive(PartialEq, Eq)]
pub enum DataType {
    Scalar,
    Vec2,
}

#[derive(Copy, Clone, Debug)]
pub enum Value {
    Vec2(Vec2),
    Scalar(Scalar),
}

#[derive(Clone)]
pub enum Node {
    ConstantScalar(Scalar),
    AddScalar(Rc<Node>, Rc<Node>),
    SubtractScalar(Rc<Node>, Rc<Node>),
    ConstantVector(Vec2),
    AddVector(Rc<Node>, Rc<Node>),
    SubtractVector(Rc<Node>, Rc<Node>),
    VectorTimesScalar(Rc<Node>, Rc<Node>),
}
