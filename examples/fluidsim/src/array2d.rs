pub type GridPos = (usize, usize);

#[derive(Clone, Debug)]
pub struct Array2D<T> {
    width: usize,
    data: Vec<T>,
}

impl<T> Array2D<T> {
    pub fn from_array(width: usize, data: Vec<T>) -> Self {
        assert_eq!(data.len() % width, 0);
        Self { width, data }
    }

    pub fn new(width: usize, height: usize) -> Self
    where
        T: Default + Copy,
    {
        Self {
            width,
            data: vec![T::default(); width * height],
        }
    }

    pub fn data(&self) -> &[T] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [T] {
        &mut self.data
    }

    fn calc_index(&self, (x, y): GridPos) -> usize {
        debug_assert!(x < self.width);
        debug_assert!(y < self.height());
        x + y * self.width
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.data.len() / self.width
    }
}

impl<T> std::ops::Index<GridPos> for Array2D<T> {
    type Output = T;
    fn index(&self, pos: GridPos) -> &T {
        &self.data[self.calc_index(pos)]
    }
}

impl<T> std::ops::IndexMut<GridPos> for Array2D<T> {
    fn index_mut(&mut self, pos: GridPos) -> &mut T {
        let idx = self.calc_index(pos);
        &mut self.data[idx]
    }
}
