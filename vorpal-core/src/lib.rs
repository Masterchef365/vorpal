use std::rc::Rc;

type Scalar = f32;
type Vec2 = [f32; 2];

#[derive(PartialEq, Eq)]
pub enum DataType {
    Scalar,
    Vec2,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Vec2(Vec2),
    Scalar(Scalar),
}

#[derive(Clone)]
pub enum Node {
    ConstantScalar(Scalar),
    AddScalar(Rc<Node>, Rc<Node>),
    SubtractScalar(Rc<Node>, Rc<Node>),
    ConstantVec2(Vec2),
    AddVec2(Rc<Node>, Rc<Node>),
    SubtractVec2(Rc<Node>, Rc<Node>),
    Vec2TimesScalar(Rc<Node>, Rc<Node>),
}

pub fn evaluate_node(node: &Node) -> Result<Value, EvalError> {
    match node {
        Node::ConstantScalar(scalar) => Ok(Value::Scalar(*scalar)),
        Node::AddScalar(a, b) => Ok(Value::Scalar(
            evaluate_node(a)?.try_to_scalar()? + evaluate_node(b)?.try_to_scalar()?,
        )),
        Node::SubtractScalar(a, b) => Ok(Value::Scalar(
            evaluate_node(a)?.try_to_scalar()? - evaluate_node(b)?.try_to_scalar()?,
        )),
        Node::ConstantVec2(scalar) => Ok(Value::Vec2(*scalar)),
        Node::AddVec2(a, b) => {
            let mut a = evaluate_node(a)?.try_to_vec2()?;
            let b = evaluate_node(b)?.try_to_vec2()?;
            a.iter_mut().zip(&b).for_each(|(a, b)| *a += b);
            Ok(Value::Vec2(a))
        }
        Node::SubtractVec2(a, b) => {
            let mut a = evaluate_node(a)?.try_to_vec2()?;
            let b = evaluate_node(b)?.try_to_vec2()?;
            a.iter_mut().zip(&b).for_each(|(a, b)| *a -= b);
            Ok(Value::Vec2(a))
        }
        Node::Vec2TimesScalar(a, b) => {
            let b = evaluate_node(&b)?.try_to_scalar()?;
            Ok(Value::Vec2(evaluate_node(a)?.try_to_vec2()?.map(|x| x * b)))
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum EvalError {
    TypeMismatch,
}

impl Value {
    /// Tries to downcast this value type to a vector
    fn try_to_vec2(self) -> Result<Vec2, EvalError> {
        match self {
            Self::Vec2(val) => Ok(val),
            Self::Scalar(_) => Err(EvalError::TypeMismatch),
        }
    }

    /// Tries to downcast this value type to a scalar
    fn try_to_scalar(self) -> Result<f32, EvalError> {
        match self {
            Self::Vec2(_) => Err(EvalError::TypeMismatch),
            Self::Scalar(val) => Ok(val),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_basic() {

        assert_eq!(
            evaluate_node(&Node::AddScalar(
                Rc::new(Node::ConstantScalar(2.0)),
                Rc::new(Node::ConstantScalar(2.0))
            )).unwrap(),
            Value::Scalar(4.0)
        );
        assert_eq!(
            evaluate_node(&Node::AddVec2(
                Rc::new(Node::ConstantVec2([2.0, -5.])),
                Rc::new(Node::ConstantVec2([2.0, 5.]))
            )).unwrap(),
            Value::Vec2([4.0, 0.0])
        );

        assert_eq!(
            evaluate_node(&Node::SubtractScalar(
                Rc::new(Node::ConstantScalar(2.0)),
                Rc::new(Node::ConstantScalar(2.0))
            )).unwrap(),
            Value::Scalar(0.0)
        );
        assert_eq!(
            evaluate_node(&Node::SubtractVec2(
                Rc::new(Node::ConstantVec2([2.0, -5.])),
                Rc::new(Node::ConstantVec2([2.0, 5.]))
            )).unwrap(),
            Value::Vec2([0.0, -10.0])
        );




    }
}
