use reed_solomon_erasure_demo::{
    codec::{Codec, RecoveryStats},
    matrix::Operations,
    shards::{join, split},
};
use std::{
    env,
    hint::black_box,
    process::ExitCode,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Options {
    compare: bool,
    k: usize,
    m: usize,
    bytes: usize,
    missing: Option<Vec<usize>>,
    seed: u64,
    iterations: usize,
}

const HELP: &str = "rs-demo [demo|compare] [options]
  --data K          data shards (demo default: 6)
  --parity M        parity shards (demo default: 3)
  --bytes N         input bytes (default: 1048576)
  --missing I,J     zero-based erased indices (demo; 'none' for no losses)
  --seed N          reproducible input seed (default: 42)
  --iterations N    timed repetitions (default: 5)
  --help            show help
compare runs 4+2, 6+3, 8+2, 10+4, deleting m shards in each.
All data stays in memory. Surviving shards are assumed correct.";

fn parse(args: &[String]) -> Result<Options> {
    let mut options = Options {
        compare: false,
        k: 6,
        m: 3,
        bytes: 1_048_576,
        missing: None,
        seed: 42,
        iterations: 5,
    };
    let mut pos = 0;
    if let Some(command) = args.first().filter(|s| !s.starts_with('-')) {
        options.compare = match command.as_str() {
            "demo" => false,
            "compare" => true,
            _ => return Err(format!("unknown command: {command}").into()),
        };
        pos = 1;
    }
    while pos < args.len() {
        let key = &args[pos];
        let value = args
            .get(pos + 1)
            .ok_or_else(|| format!("missing value for {key}"))?;
        match key.as_str() {
            "--data" | "--parity" | "--missing" if options.compare => {
                return Err(format!("{key} is only available for demo").into());
            }
            "--data" => options.k = value.parse()?,
            "--parity" => options.m = value.parse()?,
            "--bytes" => options.bytes = value.parse()?,
            "--seed" => options.seed = value.parse()?,
            "--iterations" => options.iterations = value.parse()?,
            "--missing" => {
                options.missing = Some(if value == "none" {
                    Vec::new()
                } else {
                    value
                        .split(',')
                        .map(str::parse)
                        .collect::<std::result::Result<Vec<usize>, _>>()?
                })
            }
            _ => return Err(format!("unknown option: {key}").into()),
        }
        pos += 2;
    }
    if options.iterations == 0 {
        return Err("iterations must be positive".into());
    }
    Ok(options)
}

// SplitMix64: reproducible demonstration bytes, not cryptographic randomness.
fn generate(len: usize, mut state: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(len);
    while data.len() < len {
        state = state.wrapping_add(0x9E3779B97F4A7C15);
        let mut value = state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
        value ^= value >> 31;
        for byte in value.to_le_bytes() {
            if data.len() == len {
                break;
            }
            data.push(byte);
        }
    }
    data
}

fn verify(
    shards: &[Option<Vec<u8>>],
    expected: &[Vec<u8>],
    k: usize,
    original: &[u8],
) -> Result<()> {
    if !shards
        .iter()
        .zip(expected)
        .all(|(a, b)| a.as_ref() == Some(b))
    {
        return Err("restored shards differ from encoded shards".into());
    }
    let data: Vec<_> = shards[..k]
        .iter()
        .map(|s| s.as_ref().unwrap().clone())
        .collect();
    if join(&data, original.len())? != original {
        return Err("reassembled input differs".into());
    }
    Ok(())
}

fn erase(all: &[Vec<u8>], missing: &[usize]) -> Vec<Option<Vec<u8>>> {
    all.iter()
        .enumerate()
        .map(|(i, s)| (!missing.contains(&i)).then(|| s.clone()))
        .collect()
}

fn print_ops(label: &str, ops: Operations) {
    println!(
        "  {label}: {} GF multiplications, {} matrix inversions",
        ops.gf_multiplications, ops.matrix_inversions
    );
}

fn rate(bytes: usize, seconds: f64) -> f64 {
    bytes as f64 / 1_048_576.0 / seconds
}

fn run_case(options: &Options, k: usize, m: usize, original: &[u8]) -> Result<()> {
    let counted = Codec::new::<true>(k, m)?;
    let n = k + m;
    let missing = options
        .missing
        .clone()
        .unwrap_or_else(|| (0..m).map(|i| i * n / m).collect());
    let mut sorted = missing.clone();
    sorted.sort_unstable();
    if sorted.iter().any(|&i| i >= n) {
        return Err("missing index outside shard range".into());
    }
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("duplicate missing index".into());
    }
    if missing.len() > m {
        return Err("cannot erase more than m shards".into());
    }
    let data = split(original, k)?;
    let size = data.shards[0].len();
    let (parity, encoding_ops) = counted.encode::<true>(&data.shards)?;
    let all: Vec<_> = data.shards.iter().cloned().chain(parity).collect();
    let mut damaged = erase(&all, &missing);
    let RecoveryStats {
        operations,
        data_bytes_reconstructed,
        parity_bytes_reconstructed,
    } = counted.reconstruct::<true>(&mut damaged)?;
    verify(&damaged, &all, k, original)?;

    // Warm up the uninstrumented path, then time only encoding / reconstruction.
    let codec = Codec::new::<false>(k, m)?;
    black_box(codec.encode::<false>(&data.shards)?);
    let mut warmup = erase(&all, &missing);
    codec.reconstruct::<false>(&mut warmup)?;
    let mut encode_time = Duration::ZERO;
    let mut decode_time = Duration::ZERO;
    for _ in 0..options.iterations {
        let start = Instant::now();
        let encoded = codec.encode::<false>(black_box(&data.shards))?;
        encode_time += start.elapsed();
        if encoded.0 != all[k..] {
            return Err("timed encoding differs".into());
        }
        black_box(&encoded);
        let mut shards = erase(&all, &missing);
        let start = Instant::now();
        black_box(codec.reconstruct::<false>(black_box(&mut shards))?);
        decode_time += start.elapsed();
        verify(&shards, &all, k, original)?;
    }
    let enc_secs = encode_time.as_secs_f64() / options.iterations as f64;
    let dec_secs = decode_time.as_secs_f64() / options.iterations as f64;
    let stored = size * n;
    println!("{k}+{m}: verified byte-for-byte; missing {missing:?}");
    println!(
        "  input: {} B; shard: {size} B; padding: {} B; stored: {stored} B",
        original.len(),
        size * k - original.len()
    );
    println!(
        "  parity: {} B; nominal parity overhead: {:.2}%; storage factor: {:.3}x; tolerates {m} missing shards",
        size * m,
        100.0 * m as f64 / k as f64,
        n as f64 / k as f64
    );
    print_ops("setup", counted.setup_operations());
    print_ops("encode", encoding_ops);
    print_ops("recover", operations);
    println!(
        "  reconstructed data: {data_bytes_reconstructed} B; parity: {parity_bytes_reconstructed} B (padding included)"
    );
    println!(
        "  mean of {} runs: encode {:.3} ms, {:.2} MiB/s; decode {:.3} ms, {:.2} MiB/s; repair {:.2} MiB/s",
        options.iterations,
        enc_secs * 1000.0,
        rate(original.len(), enc_secs),
        dec_secs * 1000.0,
        rate(original.len(), dec_secs),
        rate(
            data_bytes_reconstructed + parity_bytes_reconstructed,
            dec_secs
        )
    );
    Ok(())
}

fn run() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.iter().any(|s| s == "--help" || s == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    let options = parse(&args)?;
    // Validate before allocating the payload.
    if !options.compare {
        Codec::new::<false>(options.k, options.m)?;
    }
    let original = generate(options.bytes, options.seed);
    println!(
        "GF(256), polynomial 0x11D; seed {}; scalar single-threaded",
        options.seed
    );
    if cfg!(debug_assertions) {
        println!("Debug build: use --release for meaningful throughput.");
    }
    if options.compare {
        for (k, m) in [(4, 2), (6, 3), (8, 2), (10, 4)] {
            run_case(&options, k, m, &original)?;
        }
    } else {
        run_case(&options, options.k, options.m, &original)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}\nUse --help for usage.");
            ExitCode::FAILURE
        }
    }
}
