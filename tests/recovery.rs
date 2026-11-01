use reed_solomon_erasure_demo::{
    codec::{Codec, CodecError},
    matrix::{Matrix, Operations},
    shards::{join, split},
};

fn input(len: usize) -> Vec<u8> {
    (0..len).map(|i| ((i * 73 + i / 7) % 256) as u8).collect()
}

#[test]
fn exhaustive_six_plus_three() {
    let codec = Codec::new::<true>(6, 3).unwrap();
    assert_eq!(
        codec
            .coding_matrix()
            .select_rows(&(0..6).collect::<Vec<_>>())
            .unwrap(),
        Matrix::identity(6).unwrap()
    );
    let original = input(1001);
    let data = split(&original, 6).unwrap();
    let (parity, ops) = codec.encode::<true>(&data.shards).unwrap();
    assert_eq!(ops.gf_multiplications, 6 * 3 * 167);
    let all: Vec<_> = data.shards.into_iter().chain(parity).collect();
    let mut patterns = 0;
    for mask in 0u16..512 {
        if mask.count_ones() > 3 {
            continue;
        }
        patterns += 1;
        let mut shards: Vec<_> = all
            .iter()
            .enumerate()
            .map(|(i, s)| (mask & (1 << i) == 0).then(|| s.clone()))
            .collect();
        let stats = codec.reconstruct::<true>(&mut shards).unwrap();
        let restored: Vec<_> = shards.into_iter().map(Option::unwrap).collect();
        assert_eq!(restored, all, "mask={mask}");
        assert_eq!(join(&restored[..6], original.len()).unwrap(), original);
        assert_eq!(
            stats.data_bytes_reconstructed + stats.parity_bytes_reconstructed,
            mask.count_ones() as usize * 167
        );
        assert_eq!(
            stats.operations.matrix_inversions,
            u64::from(mask & 63 != 0)
        );
    }
    assert_eq!(patterns, 130);
}

#[test]
fn configurations_lengths_and_counting_modes() {
    for (k, m) in [(1, 1), (4, 2), (8, 2), (10, 4), (254, 1), (1, 254)] {
        let codec = Codec::new::<false>(k, m).unwrap();
        for len in [0, 1, 17, 1024] {
            let original = input(len);
            let data = split(&original, k).unwrap();
            let (parity, ops) = codec.encode::<false>(&data.shards).unwrap();
            assert_eq!(ops, Operations::default());
            let all: Vec<_> = data.shards.into_iter().chain(parity).collect();
            let mut shards: Vec<_> = all.iter().cloned().map(Some).collect();
            for shard in &mut shards[..m] {
                *shard = None;
            }
            let stats = codec.reconstruct::<false>(&mut shards).unwrap();
            assert_eq!(stats.operations, Operations::default());
            let restored: Vec<_> = shards.into_iter().map(Option::unwrap).collect();
            assert_eq!(restored, all);
            assert_eq!(join(&restored[..k], len).unwrap(), original);
        }
    }
}

#[test]
fn invalid_inputs_leave_shards_unchanged() {
    for (k, m) in [(0, 3), (3, 0), (255, 1), (usize::MAX, 2)] {
        assert!(matches!(
            Codec::new::<true>(k, m),
            Err(CodecError::InvalidConfiguration)
        ));
    }
    let codec = Codec::new::<true>(2, 1).unwrap();
    for (mut shards, error) in [
        (vec![Some(vec![1])], CodecError::InvalidShardCount),
        (vec![None, None, Some(vec![1])], CodecError::NotEnoughShards),
        (
            vec![Some(vec![1]), Some(vec![1, 2]), None],
            CodecError::UnequalShardLengths,
        ),
    ] {
        let before = shards.clone();
        assert_eq!(codec.reconstruct::<true>(&mut shards), Err(error));
        assert_eq!(shards, before);
    }
    assert_eq!(
        codec.encode::<true>(&[vec![1]]),
        Err(CodecError::InvalidShardCount)
    );
    assert_eq!(
        codec.encode::<true>(&[vec![1], vec![]]),
        Err(CodecError::UnequalShardLengths)
    );
    assert!(split(&[1], 0).is_err());
    assert!(join(&[vec![1]], 2).is_err());
}
