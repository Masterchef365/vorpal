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
