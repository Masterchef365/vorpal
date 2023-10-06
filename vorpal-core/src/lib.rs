use std::rc::Rc;

type Scalar = f32;
type Vec2 = [f32; 2];
type Vec3 = [f32; 3];
type Vec4 = [f32; 4];

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum DataType {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Scalar(Scalar),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
}

/// Componentwise infix operation
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ComponentInfixOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Logbase,
}

/// Function on components
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ComponentFn {
    Cosine,
    Sine,
    Tangent,
    NaturalLog,
    NaturalExp,
}

#[derive(Clone, Debug)]
pub enum Node {
    Constant(Value),
    MakeVec2([Rc<Node>; 2]),
    MakeVec3([Rc<Node>; 3]),
    MakeVec4([Rc<Node>; 4]),
    ComponentInfixOp(Rc<Node>, ComponentInfixOp, Rc<Node>),
    ComponentFn(ComponentFn, Rc<Node>),
    GetComponent(Rc<Node>, usize),
}

pub fn evaluate_node(node: &Node) -> Result<Value, EvalError> {
    match node {
        Node::Constant(value) => Ok(*value),
        _ => todo!(),
    }
}

#[derive(Copy, Clone, Debug)]
pub enum EvalError {
    TypeMismatch,
}

impl std::error::Error for EvalError {}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::TypeMismatch => write!(f, "Type mismatch"),
        }
    }
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scalar => write!(f, "Scalar"),
            Self::Vec2 => write!(f, "Vec2"),
            Self::Vec3 => write!(f, "Vec3"),
            Self::Vec4 => write!(f, "Vec4"),
        }
    }
}

impl std::fmt::Display for ComponentInfixOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Add => "add",
            Self::Multiply => "multiply",
            Self::Divide => "divide",
            Self::Subtract => "subtract",
            Self::Power => "power",
            Self::Logbase => "logbase",
        };
        write!(f, "{}", name)
    }
}

impl std::fmt::Display for ComponentFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Sine => "sine",
            Self::Cosine => "cosine",
            Self::Tangent => "tangent",
            Self::NaturalLog => "natural log",
            Self::NaturalExp => "exponential",
        };
        write!(f, "{}", name)
    }
}

impl Value {
    pub fn default_of_dtype(dtype: DataType) -> Self {
        match dtype {
            DataType::Scalar => Value::Scalar(0.0),
            DataType::Vec2 => Value::Vec2([0.0; 2]),
            DataType::Vec3 => Value::Vec3([0.0; 3]),
            DataType::Vec4 => Value::Vec4([0.0; 4]),
        }
    }
}

impl ComponentFn {
    pub fn all() -> [Self; 5] {
        [
            Self::Cosine,
            Self::Sine,
            Self::Tangent,
            Self::NaturalLog,
            Self::NaturalExp,
        ]
    }
}

impl ComponentInfixOp {
    pub fn all() -> [Self; 6] {
        [
            Self::Add,
            Self::Subtract,
            Self::Multiply,
            Self::Divide,
            Self::Power,
            Self::Logbase,
        ]
    }
}

impl DataType {
    pub fn all() -> [Self; 4] {
        [Self::Scalar, Self::Vec2, Self::Vec3, Self::Vec4]
    }

    pub fn lanes(&self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 => 4,
        }
    }
}
