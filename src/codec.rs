//! Systematic Vandermonde encoding. Surviving shards must be correct.
use crate::{
    gf256,
    matrix::{Matrix, MatrixError, Operations},
};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecError {
    InvalidConfiguration,
    InvalidShardCount,
    UnequalShardLengths,
    NotEnoughShards,
    InvalidOriginalLength,
    Matrix(MatrixError),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidConfiguration => "require k >= 1, m >= 1, k + m <= 255",
            Self::InvalidShardCount => "incorrect shard count",
            Self::UnequalShardLengths => "surviving shards must have equal lengths",
            Self::NotEnoughShards => "fewer than k surviving shards",
            Self::InvalidOriginalLength => "original length exceeds data capacity",
            Self::Matrix(_) => "invalid or singular coding matrix",
        })
    }
}
impl std::error::Error for CodecError {}
impl From<MatrixError> for CodecError {
    fn from(value: MatrixError) -> Self {
        Self::Matrix(value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecoveryStats {
    pub operations: Operations,
    pub data_bytes_reconstructed: usize,
    pub parity_bytes_reconstructed: usize,
}

pub struct Codec {
    k: usize,
    m: usize,
    generator: Matrix,
    setup_operations: Operations,
}

impl Codec {
    /// COUNT=false removes operation counters from hot loops for timing runs.
    pub fn new<const COUNT: bool>(k: usize, m: usize) -> Result<Self, CodecError> {
        let n = k.checked_add(m).ok_or(CodecError::InvalidConfiguration)?;
        if k == 0 || m == 0 || n > 255 {
            return Err(CodecError::InvalidConfiguration);
        }
        let mut ops = Operations::default();
        // Build powers by repeated multiplication so setup counting is explicit.
        let mut values = vec![0; n * k];
        for i in 0..n {
            values[i * k] = 1;
            for j in 1..k {
                values[i * k + j] = ops.multiply::<COUNT>(values[i * k + j - 1], (i + 1) as u8);
            }
        }
        let vandermonde = Matrix::new(n, k, values)?;
        let top = vandermonde.select_rows(&(0..k).collect::<Vec<_>>())?;
        let generator = vandermonde.multiply::<COUNT>(&top.invert::<COUNT>(&mut ops)?, &mut ops)?;
        Ok(Self {
            k,
            m,
            generator,
            setup_operations: ops,
        })
    }

    pub fn data_shards(&self) -> usize {
        self.k
    }
    pub fn parity_shards(&self) -> usize {
        self.m
    }
    pub fn setup_operations(&self) -> Operations {
        self.setup_operations
    }
    pub fn coding_matrix(&self) -> &Matrix {
        &self.generator
    }

    /// Returns only parity shards and encoding operation counts.
    pub fn encode<const COUNT: bool>(
        &self,
        data: &[Vec<u8>],
    ) -> Result<(Vec<Vec<u8>>, Operations), CodecError> {
        if data.len() != self.k {
            return Err(CodecError::InvalidShardCount);
        }
        let size = data[0].len();
        if data.iter().any(|s| s.len() != size) {
            return Err(CodecError::UnequalShardLengths);
        }
        let mut ops = Operations::default();
        let parity = (self.k..self.k + self.m)
            .map(|r| {
                combine::<COUNT>(
                    self.generator.row(r),
                    data.iter().map(Vec::as_slice),
                    size,
                    &mut ops,
                )
            })
            .collect();
        Ok((parity, ops))
    }

    /// Validates before mutation; restores all missing shards, including parity.
    pub fn reconstruct<const COUNT: bool>(
        &self,
        shards: &mut [Option<Vec<u8>>],
    ) -> Result<RecoveryStats, CodecError> {
        if shards.len() != self.k + self.m {
            return Err(CodecError::InvalidShardCount);
        }
        let survivors: Vec<usize> = shards
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|_| i))
            .collect();
        if survivors.len() < self.k {
            return Err(CodecError::NotEnoughShards);
        }
        let size = shards[survivors[0]].as_ref().unwrap().len();
        if shards.iter().flatten().any(|s| s.len() != size) {
            return Err(CodecError::UnequalShardLengths);
        }
        let mut stats = RecoveryStats::default();
        if shards[..self.k].iter().any(Option::is_none) {
            let selected = &survivors[..self.k];
            let inverse = self
                .generator
                .select_rows(selected)?
                .invert::<COUNT>(&mut stats.operations)?;
            let recovered: Vec<(usize, Vec<u8>)> = (0..self.k)
                .filter(|&i| shards[i].is_none())
                .map(|i| {
                    let bytes = combine::<COUNT>(
                        inverse.row(i),
                        selected
                            .iter()
                            .map(|&s| shards[s].as_ref().unwrap().as_slice()),
                        size,
                        &mut stats.operations,
                    );
                    (i, bytes)
                })
                .collect();
            for (i, bytes) in recovered {
                shards[i] = Some(bytes);
                stats.data_bytes_reconstructed += size;
            }
        }
        for i in self.k..self.k + self.m {
            if shards[i].is_none() {
                let bytes = combine::<COUNT>(
                    self.generator.row(i),
                    shards[..self.k]
                        .iter()
                        .map(|s| s.as_ref().unwrap().as_slice()),
                    size,
                    &mut stats.operations,
                );
                shards[i] = Some(bytes);
                stats.parity_bytes_reconstructed += size;
            }
        }
        Ok(stats)
    }
}

fn combine<'a, const COUNT: bool>(
    coefficients: &[u8],
    sources: impl Iterator<Item = &'a [u8]>,
    size: usize,
    ops: &mut Operations,
) -> Vec<u8> {
    let mut result = vec![0; size];
    for (&coefficient, source) in coefficients.iter().zip(sources) {
        for (dest, &byte) in result.iter_mut().zip(source) {
            *dest = gf256::add(*dest, ops.multiply::<COUNT>(coefficient, byte));
        }
    }
    result
}
