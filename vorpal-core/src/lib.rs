#[derive(PartialEq, Eq)]
pub enum DataType {
    Scalar,
    Vec2,
}

#[derive(Copy, Clone, Debug)]
pub enum Value {
    Vec2 { value: [f32; 2] },
    Scalar { value: f32 },
}
