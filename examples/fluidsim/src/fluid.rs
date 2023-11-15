pub type Array2D = crate::array2d::Array2D<f32>;

pub struct SmokeSim {
    read: Array2D,
    write: Array2D,
}

#[derive(Clone)]
pub struct FluidState {
    u: Array2D,
    v: Array2D,
}

pub struct FluidSim {
    read: FluidState,
    write: FluidState,
}

impl FluidSim {
    pub fn new(width: usize, height: usize) -> Self {
        let empty = FluidState {
            u: Array2D::new(width + 1, height),
            v: Array2D::new(width, height + 1),
        };

        Self {
            read: empty.clone(),
            write: empty,
        }
    }

    pub fn step(&mut self, dt: f32, overstep: f32, n_iters: u32) {
        // Force incompressibility
        for _ in 0..n_iters {
            for y in 1..self.read.v.height() - 2 {
                for x in 1..self.read.u.width() - 2 {
                    let dx = self.read.u[(x + 1, y)] - self.read.u[(x, y)];
                    let dy = self.read.v[(x, y + 1)] - self.read.v[(x, y)];

                    let d = overstep * (dx + dy) / 4.;

                    self.read.u[(x, y)] += d;
                    self.read.u[(x + 1, y)] -= d;

                    self.read.v[(x, y)] += d;
                    self.read.v[(x, y + 1)] -= d;
                }
            }
        }

        // Advect velocity (u component)
        for y in 1..self.read.u.height() - 1 {
            for x in 1..self.read.u.width() - 1 {
                let (px, py) = advect(&self.read.u, &self.read.v, x as f32, y as f32 + 0.5, dt);
                self.write.u[(x, y)] = interp(&self.read.u, px, py - 0.5);
            }
        }

        // Advect velocity (v component)
        for y in 1..self.read.v.height() - 1 {
            for x in 1..self.read.v.width() - 1 {
                let (px, py) = advect(&self.read.u, &self.read.v, x as f32 + 0.5, y as f32, dt);
                self.write.v[(x, y)] = interp(&self.read.v, px - 0.5, py);
            }
        }

        // Swap the written buffers back into read again
        std::mem::swap(&mut self.read.u, &mut self.write.u);
        std::mem::swap(&mut self.read.v, &mut self.write.v);
    }

    pub fn uv(&self) -> (&Array2D, &Array2D) {
        (&self.read.u, &self.read.v)
    }

    pub fn uv_mut(&mut self) -> (&mut Array2D, &mut Array2D) {
        (&mut self.read.u, &mut self.read.v)
    }

    pub fn width(&self) -> usize {
        self.read.v.width()
    }

    pub fn height(&self) -> usize {
        self.read.u.height()
    }
}

impl SmokeSim {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            read: Array2D::new(width, height),
            write: Array2D::new(width, height),
        }
    }

    pub fn advect(&mut self, (u, v): (&Array2D, &Array2D), dt: f32) {
        // Advect smoke
        for y in 1..v.height() - 2 {
            for x in 1..v.width() - 2 {
                let (px, py) = advect(&u, &v, x as f32 + 0.5, y as f32 + 0.5, dt);
                self.write[(x, y)] = interp(&self.read, px - 0.5, py - 0.5);
            }
        }

        std::mem::swap(&mut self.read, &mut self.write);
    }

    pub fn smoke(&self) -> &Array2D {
        &self.read
    }

    pub fn smoke_mut(&mut self) -> &mut Array2D {
        &mut self.read
    }
}

/// Transport x and y (relative to fluid grid coordinates) along `u` and `v` by a step `dt`
fn advect(u: &Array2D, v: &Array2D, x: f32, y: f32, dt: f32) -> (f32, f32) {
    let u = interp(&u, x, y - 0.5);
    let v = interp(&v, x - 0.5, y);

    //let [u, v, _, _] = call_kernel(u, v, x, y, dt);

    let px = x - u * dt;
    let py = y - v * dt;

    (px, py)
}

/// Linear interpolation
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (1. - t) * a + t * b
}

/// Bilinear interpolation of the given grid at the given coordinates
#[track_caller]
fn interp(grid: &Array2D, x: f32, y: f32) -> f32 {
    // Bounds enforcement. No panics!
    let tl_x = (x.floor() as isize).clamp(0, grid.width() as isize - 1) as usize;
    let tl_y = (y.floor() as isize).clamp(0, grid.height() as isize - 1) as usize;

    // Get corners
    let tl = grid[(tl_x, tl_y)];
    let tr = grid[(tl_x + 1, tl_y)];
    let bl = grid[(tl_x, tl_y + 1)];
    let br = grid[(tl_x + 1, tl_y + 1)];

    // Bilinear interpolation
    lerp(
        lerp(tl, tr, x.fract()), // Top row
        lerp(bl, br, x.fract()), // Bottom row
        y.fract(),
    )
}
