//! Dense matrices with finite-field Gauss–Jordan elimination.
use crate::gf256;

/// Counts actual multiplication calls, including zero and identity operands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Operations {
    pub gf_multiplications: u64,
    /// Number of inversion attempts.
    pub matrix_inversions: u64,
}

impl Operations {
    pub(crate) fn multiply<const COUNT: bool>(&mut self, a: u8, b: u8) -> u8 {
        if COUNT {
            self.gf_multiplications += 1;
        }
        gf256::mul(a, b)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatrixError {
    InvalidDimensions,
    Singular,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    values: Vec<u8>,
}

impl Matrix {
    pub fn new(rows: usize, cols: usize, values: Vec<u8>) -> Result<Self, MatrixError> {
        if rows == 0 || cols == 0 || rows.checked_mul(cols) != Some(values.len()) {
            return Err(MatrixError::InvalidDimensions);
        }
        Ok(Self { rows, cols, values })
    }

    pub fn identity(size: usize) -> Result<Self, MatrixError> {
        let len = size
            .checked_mul(size)
            .ok_or(MatrixError::InvalidDimensions)?;
        let mut matrix = Self::new(size, size, vec![0; len])?;
        for i in 0..size {
            matrix.values[i * size + i] = 1;
        }
        Ok(matrix)
    }

    pub fn row(&self, row: usize) -> &[u8] {
        &self.values[row * self.cols..(row + 1) * self.cols]
    }

    pub fn select_rows(&self, indices: &[usize]) -> Result<Self, MatrixError> {
        if indices.iter().any(|&i| i >= self.rows) {
            return Err(MatrixError::InvalidDimensions);
        }
        Self::new(
            indices.len(),
            self.cols,
            indices
                .iter()
                .flat_map(|&i| self.row(i).iter().copied())
                .collect(),
        )
    }

    pub fn multiply<const COUNT: bool>(
        &self,
        other: &Self,
        ops: &mut Operations,
    ) -> Result<Self, MatrixError> {
        if self.cols != other.rows {
            return Err(MatrixError::InvalidDimensions);
        }
        let len = self
            .rows
            .checked_mul(other.cols)
            .ok_or(MatrixError::InvalidDimensions)?;
        let mut values = vec![0; len];
        for r in 0..self.rows {
            for c in 0..other.cols {
                for j in 0..self.cols {
                    values[r * other.cols + c] ^=
                        ops.multiply::<COUNT>(self.row(r)[j], other.row(j)[c]);
                }
            }
        }
        Self::new(self.rows, other.cols, values)
    }

    pub fn invert<const COUNT: bool>(&self, ops: &mut Operations) -> Result<Self, MatrixError> {
        if self.rows != self.cols {
            return Err(MatrixError::InvalidDimensions);
        }
        if COUNT {
            ops.matrix_inversions += 1;
        }
        let n = self.rows;
        let mut left = self.values.clone();
        let mut right = Self::identity(n)?.values;
        for col in 0..n {
            let pivot = (col..n)
                .find(|&r| left[r * n + col] != 0)
                .ok_or(MatrixError::Singular)?;
            for j in 0..n {
                left.swap(col * n + j, pivot * n + j);
                right.swap(col * n + j, pivot * n + j);
            }
            let scale = gf256::inverse(left[col * n + col]).ok_or(MatrixError::Singular)?;
            for j in 0..n {
                left[col * n + j] = ops.multiply::<COUNT>(left[col * n + j], scale);
                right[col * n + j] = ops.multiply::<COUNT>(right[col * n + j], scale);
            }
            for r in 0..n {
                if r == col {
                    continue;
                }
                let factor = left[r * n + col];
                if factor == 0 {
                    continue;
                }
                for j in 0..n {
                    left[r * n + j] ^= ops.multiply::<COUNT>(factor, left[col * n + j]);
                    right[r * n + j] ^= ops.multiply::<COUNT>(factor, right[col * n + j]);
                }
            }
        }
        Self::new(n, n, right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_requires_row_swap_and_multiplies_to_identity() {
        let matrix = Matrix::new(3, 3, vec![0, 1, 2, 1, 0, 3, 0, 0, 1]).unwrap();
        let mut ops = Operations::default();
        let inv = matrix.invert::<true>(&mut ops).unwrap();
        assert_eq!(
            matrix.multiply::<true>(&inv, &mut ops).unwrap(),
            Matrix::identity(3).unwrap()
        );
        assert_eq!(
            inv.multiply::<true>(&matrix, &mut ops).unwrap(),
            Matrix::identity(3).unwrap()
        );
        assert_eq!(ops.matrix_inversions, 1);
    }

    #[test]
    fn rejects_singular_and_invalid_dimensions() {
        let matrix = Matrix::new(2, 2, vec![1, 2, 1, 2]).unwrap();
        assert_eq!(
            matrix.invert::<false>(&mut Operations::default()),
            Err(MatrixError::Singular)
        );
        assert_eq!(
            Matrix::new(2, 3, vec![0; 5]),
            Err(MatrixError::InvalidDimensions)
        );
        assert_eq!(Matrix::identity(0), Err(MatrixError::InvalidDimensions));
        assert_eq!(
            matrix.select_rows(&[2]),
            Err(MatrixError::InvalidDimensions)
        );
    }
}
