#[no_mangle]
pub extern "C" fn power(base: f32, exponent: f32) -> f32 {
    base.powf(exponent)
}

#[no_mangle]
pub extern "C" fn logbase(base: f32, value: f32) -> f32 {
    value.log(base)
}

#[no_mangle]
pub extern "C" fn cosine(value: f32) -> f32 {
    value.cos()
}

#[no_mangle]
pub extern "C" fn sine(value: f32) -> f32 {
    value.sin()
}

#[no_mangle]
pub extern "C" fn tangent(value: f32) -> f32 {
    value.tan()
}

#[no_mangle]
pub extern "C" fn natural_log(value: f32) -> f32 {
    value.ln()
}

#[no_mangle]
pub extern "C" fn natural_exp(value: f32) -> f32 {
    value.exp()
}

#[no_mangle]
pub extern "C" fn greater_than(lhs: f32, rhs: f32) -> f32 {
    f32::from(lhs > rhs)
}

#[no_mangle]
pub extern "C" fn less_than(lhs: f32, rhs: f32) -> f32 {
    f32::from(lhs < rhs)
}

#[no_mangle]
pub extern "C" fn equal_to(lhs: f32, rhs: f32) -> f32 {
    f32::from(lhs == rhs)
}

