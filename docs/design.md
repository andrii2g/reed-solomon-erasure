# Design and validation

## Modules

| Module | Responsibility |
|---|---|
| `gf256` | Primitive-polynomial lookup tables and scalar field arithmetic |
| `matrix` | Dense field matrices, multiplication, row selection, Gauss–Jordan inverse |
| `codec` | Normalized Vandermonde generator, parity encoding, missing-shard recovery |
| `shards` | Contiguous equal-size shards, zero padding, length-aware reassembly |
| binary | Reproducible payloads, argument validation, measurements, verification |

The crate deliberately has no third-party dependencies. GF tables are generated
at compile time. Operation counters live in a per-call `Operations` value; no
global counters or timing-dependent state are needed. Const generic counting
selects instrumented versus uninstrumented loops at compilation.

Reconstruction validates all surviving shard lengths and the survivor count
before mutation. Missing data is computed from the same selected survivor set
before inserting any recovered data. Surviving shards are preserved. The
normalized construction mathematically rules out a singular decoding matrix for
valid configurations and distinct surviving indices.

Memory is proportional to the payload and reconstructed shards, plus quadratic
matrix storage. General allocation failure is not handled. This is an educational
codec rather than a hardened storage service. Scalar encoding costs O(k*m*L),
where L is shard length; recovery costs O(k^3 + k*r*L), with r missing shards,
plus small survivor-selection work. Setup is O(k^3 + (k+m)*k^2).

## Implementation commits

1. GF(256), matrix operations, independent arithmetic tests.
2. Systematic encoding, padding/reassembly, reconstruction, exhaustive tests.
3. CLI, counters/timings, comparison cases, documentation.

## Local performance observation

Observed on Windows on 2026-11-01, rustc 1.97.1, scalar release build. Command:

```sh
cargo run --release -- compare --bytes 1048576 --iterations 10
```

These are single-process local observations, not cross-machine benchmarks.

| Data+parity | Erased indices | Parity overhead | Encode MiB/s | Decode MiB/s | Repair MiB/s |
|---|---|---:|---:|---:|---:|
| 4+2 | 0,3 | 50% | 682.29 | 731.88 | 365.94 |
| 6+3 | 0,3,6 | 50% | 500.11 | 488.93 | 244.47 |
| 8+2 | 0,5 | 25% | 752.93 | 736.75 | 184.19 |
| 10+4 | 0,3,7,10 | 40% | 376.64 | 364.57 | 145.83 |

Every run verified the complete encoded shard set and the original payload.
Different cases repair different byte counts and mixes of data/parity, so use
both decode and repair throughput when interpreting the table. CPU scheduling,
allocation, shard sizes, cache effects, and repetition count affect measurements.
No external codec performance comparison or Linux execution is claimed here.
