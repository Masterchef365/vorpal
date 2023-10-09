use std::{collections::HashMap, rc::Rc};

use ndarray::NdArray;

pub mod ndarray;

type Scalar = f32;
type Vec2 = [f32; 2];
type Vec3 = [f32; 3];
type Vec4 = [f32; 4];

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ComponentInfixOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Logbase,
}

/// Function on components
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ComponentFn {
    Cosine,
    Sine,
    Tangent,
    NaturalLog,
    NaturalExp,
}

/// Unique name of external value input
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExternInputId(String);

/// Unique name of external sampler input
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExternSamplerId(String);

#[derive(Clone, Debug)]
pub enum Node {
    ExternInput(ExternInputId),
    ExternSampler(ExternSamplerId),
    Constant(Value),
    Make(Vec<Rc<Node>>, DataType),
    ComponentInfixOp(Rc<Node>, ComponentInfixOp, Rc<Node>),
    ComponentFn(ComponentFn, Rc<Node>),
    GetComponent(Rc<Node>, Rc<Node>),
}

/// Sampler(A, B, C), samples ndarray A with a coordinate of vector B and returns vector C
pub struct Sampler(NdArray<f32>, DataType, DataType);

#[derive(Default)]
pub struct ExternContext {
    inputs: HashMap<ExternInputId, Value>,
    samplers: HashMap<ExternSamplerId, Sampler>,
}

pub fn evaluate_node(node: &Node, ctx: &ExternContext) -> Result<Value, EvalError> {
    fn comp_infix<const N: usize>(
        mut a: [f32; N],
        infix: ComponentInfixOp,
        b: [f32; N],
    ) -> [f32; N] {
        a.iter_mut()
            .zip(&b)
            .for_each(|(a, b)| *a = infix.native(*a, *b));
        a
    }

    fn comp_func<const N: usize>(mut x: [f32; N], func: ComponentFn) -> [f32; N] {
        x.iter_mut().for_each(|x| *x = func.native(*x));
        x
    }

    match node {
        Node::Make(nodes, dtype) => {
            let mut val = Value::default_of_dtype(*dtype);
            let fill = |arr: &mut [f32]| {
                for (node, out) in nodes.iter().zip(arr) {
                    let part = evaluate_node(node, ctx)?;
                    *out = part.try_into()?;
                }
                Ok(())
            };
            match &mut val {
                Value::Scalar(scalar) => {
                    let mut arr = [*scalar];
                    fill(&mut arr)?;
                    *scalar = arr[0];
                }
                Value::Vec2(arr) => fill(arr)?,
                Value::Vec3(arr) => fill(arr)?,
                Value::Vec4(arr) => fill(arr)?,
            }
            Ok(val)
        }
        Node::Constant(value) => Ok(value.clone()),
        Node::ComponentInfixOp(a, op, b) => {
            match (evaluate_node(a, ctx)?, evaluate_node(b, ctx)?) {
                (Value::Scalar(a), Value::Scalar(b)) => {
                    Ok(Value::Scalar(comp_infix([a], *op, [b])[0]))
                }
                (Value::Vec2(a), Value::Vec2(b)) => Ok(Value::Vec2(comp_infix(a, *op, b))),
                (Value::Vec3(a), Value::Vec3(b)) => Ok(Value::Vec3(comp_infix(a, *op, b))),
                (Value::Vec4(a), Value::Vec4(b)) => Ok(Value::Vec4(comp_infix(a, *op, b))),
                _ => Err(EvalError::TypeMismatch),
            }
        }
        Node::ComponentFn(func, a) => match evaluate_node(a, ctx)? {
            Value::Scalar(a) => Ok(Value::Scalar(comp_func([a], *func)[0])),
            Value::Vec2(a) => Ok(Value::Vec2(comp_func(a, *func))),
            Value::Vec3(a) => Ok(Value::Vec3(comp_func(a, *func))),
            Value::Vec4(a) => Ok(Value::Vec4(comp_func(a, *func))),
        },
        Node::GetComponent(value, index) => {
            let value = evaluate_node(value, ctx)?;
            if let Value::Scalar(index) = evaluate_node(index, ctx)? {
                let index = index.clamp(0., value.dtype().lanes() as f32);
                let index = (index as usize).clamp(0, value.dtype().lanes() - 1);
                Ok(Value::Scalar(match value {
                    Value::Scalar(val) => val,
                    Value::Vec2(arr) => arr[index],
                    Value::Vec3(arr) => arr[index],
                    Value::Vec4(arr) => arr[index],
                }))
            } else {
                Err(EvalError::TypeMismatch)
            }
        }
        Node::ExternInput(id) => ctx
            .inputs
            .get(id)
            .copied()
            .ok_or_else(|| EvalError::BadInputId(id.clone())),
        _ => todo!(),
    }
}

#[derive(Clone, Debug)]
pub enum EvalError {
    TypeMismatch,
    BadInputId(ExternInputId),
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

impl DataType {
    pub fn dtype_name(&self) -> &'static str {
        match self {
            Self::Scalar => "Scalar",
            Self::Vec2 => "Vec2",
            Self::Vec3 => "Vec3",
            Self::Vec4 => "Vec4",
        }
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
    pub fn all() -> [Self; 5] {
        [
            Self::Cosine,
            Self::Sine,
            Self::Tangent,
            Self::NaturalLog,
            Self::NaturalExp,
        ]
    }

    pub fn native(&self, x: f32) -> f32 {
        match self {
            Self::Cosine => x.cos(),
            Self::Sine => x.sin(),
            Self::Tangent => x.tan(),
            Self::NaturalLog => x.ln(),
            Self::NaturalExp => x.exp(),
        }
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

    pub fn native(&self, a: f32, b: f32) -> f32 {
        match self {
            Self::Add => a + b,
            Self::Subtract => a - b,
            Self::Multiply => a * b,
            Self::Divide => a / b,
            Self::Power => a.powf(b),
            Self::Logbase => a.log(b),
        }
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
