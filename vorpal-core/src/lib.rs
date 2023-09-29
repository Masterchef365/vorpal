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
    Constant(Value),
    MakeVec2(Rc<Node>, Rc<Node>),
    AddScalar(Rc<Node>, Rc<Node>),
    SubtractScalar(Rc<Node>, Rc<Node>),
    AddVec2(Rc<Node>, Rc<Node>),
    SubtractVec2(Rc<Node>, Rc<Node>),
    Vec2TimesScalar(Rc<Node>, Rc<Node>),
}

pub fn evaluate_node(node: &Node) -> Result<Value, EvalError> {
    match node {
        Node::Constant(value) => Ok(*value),
        Node::AddScalar(a, b) => Ok(Value::Scalar(
            evaluate_node(a)?.try_to_scalar()? + evaluate_node(b)?.try_to_scalar()?,
        )),
        Node::SubtractScalar(a, b) => Ok(Value::Scalar(
            evaluate_node(a)?.try_to_scalar()? - evaluate_node(b)?.try_to_scalar()?,
        )),
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
        Node::MakeVec2(a, b) => Ok(Value::Vec2([
            evaluate_node(a)?.try_to_scalar()?,
            evaluate_node(b)?.try_to_scalar()?,
        ])),
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
                Rc::new(Node::Constant(Value::Scalar(2.0))),
                Rc::new(Node::Constant(Value::Scalar(2.0)))
            ))
            .unwrap(),
            Value::Scalar(4.0)
        );
        assert_eq!(
            evaluate_node(&Node::AddVec2(
                Rc::new(Node::Constant(Value::Vec2([2.0, -5.]))),
                Rc::new(Node::Constant(Value::Vec2([2.0, 5.])))
            ))
            .unwrap(),
            Value::Vec2([4.0, 0.0])
        );

        assert_eq!(
            evaluate_node(&Node::SubtractScalar(
                Rc::new(Node::Constant(Value::Scalar(2.0))),
                Rc::new(Node::Constant(Value::Scalar(2.0)))
            ))
            .unwrap(),
            Value::Scalar(0.0)
        );
        assert_eq!(
            evaluate_node(&Node::SubtractVec2(
                Rc::new(Node::Constant(Value::Vec2([2.0, -5.]))),
                Rc::new(Node::Constant(Value::Vec2([2.0, 5.])))
            ))
            .unwrap(),
            Value::Vec2([0.0, -10.0])
        );
    }
}

impl std::error::Error for EvalError {}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::TypeMismatch => write!(f, "Type mismatch"),
        }
    }
}
