use std::cell::RefCell;

#[link(wasm_import_module = "kernel")]
extern "C" {
    fn kernel(ptr: *mut f32, width: f32, height: f32, x: f32, y: f32, time: f32);
}

pub fn call_kernel(width: f32, height: f32, x: f32, y: f32, time: f32) -> [f32; 4] {
    let mut out_data = [0_f32; 4];

    unsafe {
        kernel(out_data.as_mut_ptr(), width, height, x, y, time);
    }

    out_data
}

#[no_mangle]
pub extern "C" fn make_image(width: u32, height: u32, time: f32) -> *const f32 {
    thread_local! {
        static BUFFER: RefCell<Option<Plugin>> = RefCell::new(None);
    }

    BUFFER.with(|buffer| {
        let mut maybe_plugin = buffer.borrow_mut();
        let plugin = maybe_plugin.get_or_insert_with(|| Plugin::new(width, height));

        plugin.get_image(time).as_ptr()
    })
}

struct Plugin {
    out_rgba: Vec<f32>,
    out_width: usize,
    out_height: usize,
}

impl Plugin {
    pub fn new(out_width: u32, out_height: u32) -> Self {
        Self {
            out_rgba: vec![0_f32; (out_width * out_height * 4) as usize],
            out_width: out_width as _,
            out_height: out_height as _,
        }
    }

    pub fn get_image(&mut self, time: f32) -> &[f32] {
        for y in 0..self.out_height {
            for x in 0..self.out_width {
                let idx = 4 * (y * self.out_width + x);

                let sx = x as i32 - self.out_width as i32/2;
                let sy = y as i32 - self.out_height as i32/2;
                if sx*sx + sy*sy < 30 {
                    self.out_rgba[idx..idx+4].copy_from_slice(&[0., 1., 2., 3.]);
                }
            }
        }

        &self.out_rgba
    }
}
