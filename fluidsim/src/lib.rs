use std::cell::RefCell;

use array2d::Array2D;
use fluid::{FluidSim, SmokeSim};

mod array2d;
mod fluid;

/// This is the "main" function, generating an image and saving it in a
/// place we may retrieve it later from the outside
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

#[link(wasm_import_module = "add_velocity")]
extern "C" {
    fn add_velocity(
        ptr: *mut f32,
        cursor_x: f32,
        cursor_y: f32,
        last_cursor_x: f32,
        last_cursor_y: f32,
        pos_x: f32,
        pos_y: f32,
        resolution_x: f32,
        resolution_y: f32,
        time: f32,
    );
}

#[link(wasm_import_module = "get_color")]
extern "C" {
    fn get_color(
        ptr: *mut f32,
        cursor_x: f32,
        cursor_y: f32,
        fluid_vel_x: f32,
        fluid_vel_y: f32,
        pos_x: f32,
        pos_y: f32,
        resolution_x: f32,
        resolution_y: f32,
        smoke_quantity: f32,
        time: f32,
    );
}

struct Plugin {
    out_rgba: Vec<f32>,

    smoke_sim: SmokeSim,
    fluid_sim: FluidSim,
    last_cursor: Option<[f32; 2]>,
}

impl Plugin {
    pub fn new(out_width: u32, out_height: u32) -> Self {
        assert_eq!(out_width, out_height);
        let w = out_width as usize;
        let fluid_sim = FluidSim::new(w, w);
        let mut smoke_sim = SmokeSim::new(w, w);

        let intensity = 1e4;
        smoke_sim.smoke_mut()[(w / 2, w / 3)] = intensity;

        Self {
            out_rgba: vec![0_f32; (out_width * out_height * 4) as usize],

            fluid_sim,
            smoke_sim,
            last_cursor: None,
        }
    }

    pub fn get_image(&mut self, time: f32, cursor_x: f32, cursor_y: f32) -> &[f32] {
        // Add forces
        let (w, h) = (self.fluid_sim.width(), self.fluid_sim.height());
        for y in 0..h {
            for x in 0..w {
                let (u, v) = self.fluid_sim.uv_mut();
                let [last_cursor_x, last_cursor_y] =
                    self.last_cursor.unwrap_or([cursor_x, cursor_y]);

                let mut out_vals = [0.; 4];
                unsafe {
                    add_velocity(
                        out_vals.as_mut_ptr(),
                        cursor_x,
                        cursor_y,
                        last_cursor_x,
                        last_cursor_y,
                        x as f32,
                        y as f32,
                        w as f32,
                        h as f32,
                        time,
                    );
                }

                let [add_u, add_v, add_smoke, _] = out_vals;

                self.smoke_sim.smoke_mut()[(x, y)] += add_smoke;
                u[(x, y)] += add_u;
                v[(x, y)] += add_v;
            }
        }

        // Move fluid and smoke
        let dt = 1e-2;
        let overstep = 1.9;

        self.fluid_sim.step(dt, overstep, 15);
        self.smoke_sim.advect(self.fluid_sim.uv(), dt);

        let (u, v) = self.fluid_sim.uv();

        // Build output buffer
        let mut output_buf: Array2D<[f32; 4]> = Array2D::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let vel_x = u[(x, y)];
                let vel_y = v[(x, y)];
                let smoke_quantity = self.smoke_sim.smoke()[(x, y)];

                let mut rgba = [0.; 4];
                unsafe {
                    get_color(
                        rgba.as_mut_ptr(),
                        cursor_x,
                        cursor_y,
                        vel_x,
                        vel_y,
                        x as f32,
                        y as f32,
                        w as f32,
                        h as f32,
                        smoke_quantity,
                        time,
                    );
                }

                output_buf[(x, y)] = rgba;
            }
        }

        self.out_rgba.clear();
        self.out_rgba.extend(output_buf.data().iter().flatten());

        if cursor_x > 0. && cursor_y > 0. {
            self.last_cursor = Some([cursor_x, cursor_y]);
        }

        &self.out_rgba
    }
}
