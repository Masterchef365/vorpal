use std::{collections::HashMap, rc::Rc};

use ndarray::NdArray;

pub mod native_backend;
pub mod ndarray;

pub type Scalar = f32;
pub type Vec2 = [f32; 2];
pub type Vec3 = [f32; 3];
pub type Vec4 = [f32; 4];

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum DataType {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Scalar(Scalar),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
}

/// Componentwise infix operation
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ComponentInfixOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Logbase,
    GreaterThan,
    LessThan,
}

/// Function on components
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ComponentFn {
    Cosine,
    Sine,
    Tangent,
    NaturalLog,
    NaturalExp,
    Ceil,
    Floor,
    Abs,
}

#[derive(Clone, Debug)]
pub enum EvalError {
    TypeMismatch,
    BadInputId(ExternInputId),
}

/// Unique name of external value input
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExternInputId(String);

/// Unique name of external sampler input
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExternSamplerId(String);

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    ExternInput(ExternInputId, DataType),
    ExternSampler(ExternSamplerId),
    Constant(Value),
    Make(Vec<Rc<Node>>, DataType),
    ComponentInfixOp(Rc<Node>, ComponentInfixOp, Rc<Node>),
    ComponentFn(ComponentFn, Rc<Node>),
    GetComponent(Rc<Node>, Rc<Node>),
    Dot(Rc<Node>, Rc<Node>),
}

/// Sampler(A, B, C), samples ndarray A with a coordinate of vector B and returns vector C
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct Sampler(NdArray<f32>, DataType, DataType);

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct ExternContext {
    pub inputs: HashMap<ExternInputId, Value>,
    pub samplers: HashMap<ExternSamplerId, Sampler>,
}

impl DataType {
    pub fn dtype_name(&self) -> &'static str {
        match self {
            Self::Scalar => "Scalar",
            Self::Vec2 => "Vec2",
            Self::Vec3 => "Vec3",
            Self::Vec4 => "Vec4",
        }
    }

    pub fn lane_names(&self) -> impl Iterator<Item = char> {
        "xyzw".chars().take(self.n_lanes())
    }
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.dtype_name())
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
            Self::GreaterThan => "greater than",
            Self::LessThan => "less than",
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
            Self::Abs => "absolute value",
            Self::Ceil => "ceiling",
            Self::Floor => "floor",
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

    pub fn dtype(&self) -> DataType {
        match self {
            Self::Scalar(_) => DataType::Scalar,
            Self::Vec2(_) => DataType::Vec2,
            Self::Vec3(_) => DataType::Vec3,
            Self::Vec4(_) => DataType::Vec4,
        }
    }
}

macro_rules! impl_value_try_into {
    ($target_type:ty, $enum_variant:ident) => {
        impl std::convert::TryInto<$target_type> for Value {
            type Error = EvalError;

            fn try_into(self) -> Result<$target_type, Self::Error> {
                match self {
                    Value::$enum_variant(value) => Ok(value),
                    _ => Err(EvalError::TypeMismatch),
                }
            }
        }
    };
}

impl_value_try_into!(f32, Scalar);
impl_value_try_into!(Vec2, Vec2);
impl_value_try_into!(Vec3, Vec3);
impl_value_try_into!(Vec4, Vec4);

impl ComponentFn {
    pub fn all() -> [Self; 8] {
        [
            Self::Cosine,
            Self::Sine,
            Self::Tangent,
            Self::NaturalLog,
            Self::NaturalExp,
            Self::Ceil,
            Self::Floor,
            Self::Abs,
        ]
    }

    pub fn native(&self, x: f32) -> f32 {
        match self {
            Self::Cosine => x.cos(),
            Self::Sine => x.sin(),
            Self::Tangent => x.tan(),
            Self::NaturalLog => x.ln(),
            Self::NaturalExp => x.exp(),
            Self::Ceil => x.ceil(),
            Self::Floor => x.floor(),
            Self::Abs => x.abs(),
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Cosine => "cos",
            Self::Sine => "sin",
            Self::Tangent => "tan",
            Self::NaturalLog => "ln",
            Self::NaturalExp => "e^",
            Self::Ceil => "ceil",
            Self::Floor => "floor",
            Self::Abs => "abs",
        }
    }
}

impl ComponentInfixOp {
    pub fn all() -> [Self; 8] {
        [
            Self::Add,
            Self::Subtract,
            Self::Multiply,
            Self::Divide,
            Self::Power,
            Self::Logbase,
            Self::GreaterThan,
            Self::LessThan,
        ]
    }

    pub fn native(&self, a: f32, b: f32) -> f32 {
        match self {
            Self::Add => a + b,
            Self::Subtract => a - b,
            Self::Multiply => a * b,
            Self::Divide => a / b,
            Self::Power => a.powf(b),
            Self::Logbase => a.log(b),
            Self::GreaterThan => f32::from(a > b),
            Self::LessThan => f32::from(a < b),
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Power => "^",
            Self::Logbase => "logbase",
            Self::GreaterThan => ">",
            Self::LessThan => "<",
        }
    }
}

impl DataType {
    pub fn all() -> [Self; 4] {
        [Self::Scalar, Self::Vec2, Self::Vec3, Self::Vec4]
    }

    pub fn n_lanes(&self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 => 4,
        }
    }
}

/*
impl Sampler {
    pub fn sample(&self, coord: Value) -> Value {
        let Self(array, input_dtype, output_dtype) = self;
        assert_eq!(coord.dtype(), input_dtype);
        let mut index_array = [0_usize; 8];
        let mut index_array_ptr = 0;

        match coord {
            Value::Scalar(scalar) => {
                index_array[]
            }
        }

        //index_array[index_array_ptr] =
    }
}
*/

impl std::fmt::Display for ExternInputId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(name) = self;
        name.fmt(f)
    }
}

impl std::fmt::Display for ExternSamplerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(name) = self;
        name.fmt(f)
    }
}

impl ExternInputId {
    pub fn new(name: String) -> Self {
        Self(name)
    }
}

impl ExternSamplerId {
    pub fn new(name: String) -> Self {
        Self(name)
    }
}

impl ExternContext {
    pub fn new(
        inputs: HashMap<ExternInputId, Value>,
        samplers: HashMap<ExternSamplerId, Sampler>,
    ) -> Self {
        Self { inputs, samplers }
    }

    pub fn inputs(&self) -> &HashMap<ExternInputId, Value> {
        &self.inputs
    }

    pub fn samplers(&self) -> &HashMap<ExternSamplerId, Sampler> {
        &self.samplers
    }

    pub fn insert_input(&mut self, id: &ExternInputId, value: Value) {
        if let Some(inner_val) = self.inputs.get(id) {
            assert_eq!(inner_val.dtype(), value.dtype());
        }
        self.inputs.insert(id.clone(), value);
    }

    pub fn set_sampler(&mut self, _id: &ExternSamplerId, _value: Sampler) {
        /*Sampler(array, )
        //assert_eq!(self.inputs[id].dtype(), value.dtype());
        self.inputs.insert(id.clone(), value);
        */
        todo!()
    }
}

impl std::error::Error for EvalError {}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::TypeMismatch => write!(f, "Type mismatch"),
            EvalError::BadInputId(id) => write!(f, "Bad input id: {:?}", id),
        }
    }
}

impl Value {
    pub fn iter_vector_floats(self) -> impl Iterator<Item = f32> {
        let mut i = 0;
        std::iter::from_fn(move || {
            let result = match self {
                Self::Scalar(val) => (i == 0).then(|| val),
                Self::Vec2(val) => val.get(i).copied(),
                Self::Vec3(val) => val.get(i).copied(),
                Self::Vec4(val) => val.get(i).copied(),
            };
            i += 1;
            result
        })
    }
}
