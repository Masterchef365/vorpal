use std::cell::RefCell;

#[link(wasm_import_module = "kernel")]
extern "C" {
    #[allow(improper_ctypes)] // [this is fine]
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

                /*
                let r = x as f32 / width as f32;
                let g = y as f32 / height as f32;
                let b = time.cos();
                let a = 0.;
                */

                let base = (x * 4 + y * width * 4) as usize;
                image[base + 0] = r;
                image[base + 1] = g;
                image[base + 2] = b;
                image[base + 3] = a;
            }
        }

        image.as_ptr()
    })
}
