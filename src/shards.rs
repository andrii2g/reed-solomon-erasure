//! Equal-size contiguous shards; original length is out-of-band metadata.
use crate::codec::CodecError;

#[derive(Clone, Debug)]
pub struct DataShards {
    pub shards: Vec<Vec<u8>>,
    pub original_len: usize,
}

pub fn split(input: &[u8], k: usize) -> Result<DataShards, CodecError> {
    if !(1..=255).contains(&k) {
        return Err(CodecError::InvalidConfiguration);
    }
    let size = input.len().div_ceil(k);
    let mut shards = vec![vec![0; size]; k];
    if size != 0 {
        for (i, chunk) in input.chunks(size).enumerate() {
            shards[i][..chunk.len()].copy_from_slice(chunk);
        }
    }
    Ok(DataShards {
        shards,
        original_len: input.len(),
    })
}

pub fn join(data: &[Vec<u8>], original_len: usize) -> Result<Vec<u8>, CodecError> {
    let size = data.first().ok_or(CodecError::InvalidShardCount)?.len();
    if data.iter().any(|s| s.len() != size) {
        return Err(CodecError::UnequalShardLengths);
    }
    let capacity = size
        .checked_mul(data.len())
        .ok_or(CodecError::InvalidOriginalLength)?;
    if original_len > capacity {
        return Err(CodecError::InvalidOriginalLength);
    }
    Ok(data.iter().flatten().copied().take(original_len).collect())
}
