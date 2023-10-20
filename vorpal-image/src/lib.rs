use std::cell::RefCell;

#[link(wasm_import_module = "kernel")]
extern "C" {
    fn special_image_function(ptr: *mut f32, width: f32, height: f32, x: f32, y: f32, time: f32);
}

fn call_image_fn(width: f32, height: f32, x: f32, y: f32, time: f32) -> [f32; 4] {
    {
        let mut out_data = [0_f32; 4];

        unsafe {
            special_image_function(out_data.as_mut_ptr(), width, height, x, y, time);
        }

        out_data
    }
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
                let rgba = call_image_fn(width as f32, height as f32, x as f32, y as f32, time);

                /*
                let r = x as f32 / width as f32;
                let g = y as f32 / height as f32;
                let b = time.cos();
                let a = 0.;
                */

                let base = (x * 4 + y * width * 4) as usize;
                image[base..base + 4].copy_from_slice(&rgba);
            }
        }

        image.as_ptr()
    })
}
