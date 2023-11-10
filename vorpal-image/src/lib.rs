use std::cell::RefCell;

#[link(wasm_import_module = "kernel")]
extern "C" {
    fn kernel(
        ptr: *mut f32,
        cursor_x: f32,
        cursor_y: f32,
        position_x: f32,
        position_y: f32,
        width: f32,
        height: f32,
        time: f32,
    );
}

pub fn call_kernel(
    resolution: [f32; 2],
    position: [f32; 2],
    time: f32,
    cursor_pos: [f32; 2],
) -> [f32; 4] {
    let mut out_data = [0_f32; 4];
    let [width, height] = resolution;
    let [x, y] = position;
    let [cursor_x, cursor_y] = cursor_pos;
    unsafe {
        kernel(
            out_data.as_mut_ptr(),
            cursor_x,
            cursor_y,
            x,
            y,
            width,
            height,
            time,
        );
    }

    out_data
}

#[no_mangle]
pub extern "C" fn make_image(
    width: u32,
    height: u32,
    time: f32,
    cursor_x: f32,
    cursor_y: f32,
) -> *const f32 {
    thread_local! {
        static BUFFER: RefCell<Option<Plugin>> = RefCell::new(None);
    }

    BUFFER.with(|buffer| {
        let mut maybe_plugin = buffer.borrow_mut();
        let plugin = maybe_plugin.get_or_insert_with(|| Plugin::new(width, height));

        plugin.get_image(time, cursor_x, cursor_y).as_ptr()
    })
}

struct Plugin {
    out_rgba: Vec<f32>,
    out_width: u32,
    out_height: u32,
}

impl Plugin {
    pub fn new(out_width: u32, out_height: u32) -> Self {
        Self {
            out_rgba: vec![0_f32; (out_width * out_height * 4) as usize],
            out_width,
            out_height,
        }
    }

    pub fn get_image(&mut self, time: f32, cursor_x: f32, cursor_y: f32) -> &[f32] {
        for y in 0..self.out_height {
            for x in 0..self.out_width {
                let [sx, sy] = [x, y].map(|v| v as f32);

                let rgba = call_kernel(
                    [self.out_width as f32, self.out_height as f32],
                    [sx, sy],
                    time,
                    [cursor_x, cursor_y],
                );

                let idx = 4 * (y * self.out_width + x) as usize;
                self.out_rgba[idx..idx + 4].copy_from_slice(&rgba);
            }
        }

        &self.out_rgba
    }
}
