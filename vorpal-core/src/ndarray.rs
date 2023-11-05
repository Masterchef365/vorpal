use std::ops::{Index, IndexMut};

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct NdArray<T> {
    dims: Vec<usize>,
    data: Vec<T>,
}

impl<T> NdArray<T> {
    pub fn zeros(dims: Vec<usize>) -> Self
    where
        T: Default + Clone,
    {
        let n_elems = dims.iter().product();
        Self {
            dims,
            data: vec![T::default(); n_elems],
        }
    }

    #[track_caller]
    pub fn calc_index(&self, index_nd: &[usize]) -> Option<usize> {
        assert_eq!(
            index_nd.len(),
            self.dims.len(),
            "Index ({:?}) had {} dimensions, Array size ({:?}) has {}",
            index_nd,
            index_nd.len(),
            self.dims,
            self.dims.len()
        );

        let mut index_1d = 0;
        let mut stride = 1;
        // Note: Index is iterated over in reverse order so that adding 1 to the last index coord
        // adds 1 to the array index
        for (extent, dim_size) in index_nd.iter().zip(&self.dims).rev() {
            if extent > dim_size {
                return None;
            }
            index_1d += extent * stride;
            stride *= dim_size;
        }
        Some(index_1d)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[track_caller]
    pub fn calc_index_or_panic(&self, index_nd: &[usize]) -> usize {
        if let Some(idx) = self.calc_index(index_nd) {
            idx
        } else {
            panic!(
                "Index ({:?}) out of bounds for array size ({:?})",
                index_nd, self.dims,
            );
        }
    }

    pub fn shape(&self) -> &[usize] {
        &self.dims
    }

    pub fn data(&self) -> &[T] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [T] {
        &mut self.data
    }
}

impl<const N: usize, T> Index<[usize; N]> for NdArray<T> {
    type Output = T;
    fn index(&self, index_nd: [usize; N]) -> &Self::Output {
        let idx_1d = self.calc_index_or_panic(&index_nd);
        &self.data[idx_1d]
    }
}

impl<const N: usize, T> IndexMut<[usize; N]> for NdArray<T> {
    fn index_mut(&mut self, index_nd: [usize; N]) -> &mut Self::Output {
        let idx_1d = self.calc_index_or_panic(&index_nd);
        &mut self.data[idx_1d]
    }
}
