//! Minimal CSR (Compressed Sparse Row) matrix for synapse weights (fixed-point)

use crate::fixed::FRACTIONAL_BITS;

pub struct CsrMatrix {
    pub row_ptr: Vec<usize>,
    pub col_idx: Vec<usize>,
    pub values: Vec<i32>, // fixed-point weights (Q16.16 by default)
}

impl CsrMatrix {
    pub fn new(row_ptr: Vec<usize>, col_idx: Vec<usize>, values: Vec<i32>) -> Self {
        debug_assert!(row_ptr.len() >= 1);
        debug_assert!(col_idx.len() == values.len());
        Self { row_ptr, col_idx, values }
    }

    /// y = A * x (both fixed-point vectors); returns fixed-point vector
    pub fn mul_vector(&self, x: &[i32]) -> Vec<i32> {
        let rows = self.row_ptr.len() - 1;
        let mut result = vec![0i32; rows];
        for r in 0..rows {
            let start = self.row_ptr[r];
            let end = self.row_ptr[r + 1];
            let mut acc: i64 = 0;
            for k in start..end {
                let c = self.col_idx[k];
                acc += (self.values[k] as i64) * (x[c] as i64);
            }
            result[r] = (acc >> FRACTIONAL_BITS) as i32; // rescale from fixed-point mul
        }
        result
    }
}