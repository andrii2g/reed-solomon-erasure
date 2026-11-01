# Reed–Solomon erasure coding in Rust

A dependency-free educational implementation of systematic Reed–Solomon erasure
coding over GF(256). Recover missing data and parity shards from any `k` surviving
shards out of `k + m`. All operations run in memory.

## Run

Install stable Rust (edition 2024 requires Rust 1.85 or later), then:

```sh
cargo run --release -- demo --data 6 --parity 3 --bytes 1048576 --missing 0,4,7 --seed 42
cargo run --release -- compare --bytes 1048576 --iterations 10
cargo run --release -- demo --missing none
cargo run -- --help
```

Shard indices are zero-based: data occupies `0..k`, parity occupies `k..k+m`.
With no explicit loss pattern, the CLI deletes `m` evenly spaced shard indices.
`compare` runs 4+2, 6+3, 8+2, and 10+4 using the same generated input. Repeat it
with `--bytes 4096`, `65536`, and `1048576` to explore payload/shard-size effects.
`--seed` controls reproducible SplitMix64 input bytes, not cryptographic randomness.

The CLI verifies every restored shard and the reassembled original input. Invalid
arguments, too many erasures, or failed recovery produce a nonzero exit status.

## Library example

```rust
use reed_solomon_erasure_demo::{codec::Codec, shards::{split, join}};

let input = b"Finite fields recover missing fragments.";
let codec = Codec::new::<true>(6, 3)?;
let data = split(input, codec.data_shards())?;
let (parity, encode_ops) = codec.encode::<true>(&data.shards)?;
let mut shards: Vec<_> = data.shards.into_iter().chain(parity).map(Some).collect();
for index in [0, 4, 7] {
    shards[index] = None;
}
let stats = codec.reconstruct::<true>(&mut shards)?;
let restored: Vec<_> = shards[..6].iter().map(|s| s.clone().unwrap()).collect();
assert_eq!(join(&restored, data.original_len)?, input);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`::<true>` counts GF multiplications and inversion attempts. `::<false>` compiles
out counter increments for timing. Both modes use the same coding algorithm.
Constructing a codec is separate from encoding/recovery and can be reused.
The original input length is out-of-band metadata; retain it to remove padding.
Empty input is supported with zero-length shards.

## Mathematics

Addition and subtraction are XOR. Multiplication uses logarithm/exponent tables
with primitive polynomial `0x11D` and primitive element `0x02`. Zero is handled
separately; division by zero and the inverse of zero return `None`.

For `n = k + m`, use distinct nonzero field elements `x_i = i + 1` and construct
`V[i,j] = x_i^j`. If `A` is the top `k × k` block, the generator is `G = V A^-1`.
Its top block is the identity, leaving the original data shards unchanged.
For any `k` surviving indices, the corresponding Vandermonde block is invertible:
its determinant is a product of nonzero differences of distinct evaluation points.
Multiplying by `A^-1` preserves invertibility. Thus any `k` rows of `G` suffice.
This is the normalized Vandermonde construction used by
[Backblaze's reference implementation](https://github.com/Backblaze/JavaReedSolomon/blob/master/src/main/java/com/backblaze/erasure/ReedSolomon.java).

Encoding applies the parity rows independently at every byte offset. Recovery
selects the first `k` surviving rows, inverts that matrix once using Gauss–Jordan
elimination, and applies the required inverse rows to recover missing data.
Missing parity is then regenerated. Parity-only loss requires no matrix inverse.
Configurations require `k >= 1`, `m >= 1`, and `k + m <= 255`.

## Measurements

- Nominal parity overhead: `m / k`; nominal storage factor: `(k + m) / k`.
  The CLI also reports actual stored bytes and data padding separately.
- Missing shards tolerated: `m`, assuming every survivor is correct.
- GF multiplications: actual multiplication calls, including zero and identity
  operands, separated into setup, encoding, and recovery. Inverse table lookups,
  XORs, and memory operations are not counted as multiplications.
- Matrix inversions: attempts, not row operations; one during codec setup,
  at most one during a recovery call.
- Encode/decode throughput: original input bytes divided by elapsed seconds.
  Decode includes restoring missing data and parity, selecting survivors,
  allocating outputs, and matrix inversion.
- Repair throughput: restored data plus parity bytes divided by recovery time.
  Restored byte counts include padding.

The CLI performs a counted correctness run, warms up an uninstrumented codec,
then reports the arithmetic mean time across `--iterations` runs. Input generation,
splitting, shard deletion, verification, and codec setup are outside timed regions.
Encoding output allocation and recovery output allocation are inside them.
Throughput is in MiB/s (1 MiB = 1,048,576 bytes). Use release builds. Results are
local observations rather than performance guarantees; no-loss and empty-input
cases are correctness demonstrations, not useful decode benchmarks.

See [design and validation](docs/design.md) for the module map and baseline results.

## Verify

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --release
cargo test --doc
```

Tests cover all 65,536 GF products against an independent polynomial multiplier,
all nonzero inverses, row-swapping matrix inversion, and all 130 patterns of up to
three erasures for 6+3 (including all 84 selections of six survivors). Further tests
cover empty and padded inputs, parity-only/data-only/mixed loss, maximum shard
counts, malformed input without mutation, and CLI behavior. CI runs on Linux and
Windows with stable Rust.

This project handles known erasures only. It assumes surviving shards are correct
and does not detect or locate corrupt bytes. It has no filesystem storage layer,
networking, SIMD, threading, or general error-correction decoder.
