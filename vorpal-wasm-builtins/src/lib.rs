use std::{borrow::BorrowMut, cell::RefCell};

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

#[link(wasm_import_module = "special")]
extern "C" {
    fn special_image_function(
        width: f32,
        height: f32,
        x: f32,
        y: f32,
        time: f32,
    ) -> (f32, f32, f32, f32);
}

#[no_mangle]
pub extern "C" fn make_image(width: u32, height: u32, time: f32) -> *const f32 {
    thread_local! {
        static BUFFER: RefCell<Vec<f32>> = RefCell::new(Vec::new());
    }

    BUFFER.with(|buffer| {
        let mut image = buffer.borrow_mut();
        *image = vec![0_f32; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let (r, g, b, a) = unsafe {
                    special_image_function(width as f32, height as f32, x as f32, y as f32, time)
                };
                let base = (x * 4 + y * width * 4 + 0) as usize;
                image[base + 0] = r;
                image[base + 1] = g;
                image[base + 2] = b;
                image[base + 3] = a;
            }
        }

        image.as_ptr()
    })
}
