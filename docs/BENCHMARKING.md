# Benchmarking

base-d uses [Criterion](https://github.com/bheisler/criterion.rs) for statistical benchmarking.

## Quick Start

```bash
# Run full benchmark suite
cargo bench --bench encoding

# Run specific dictionary
cargo bench --bench encoding -- "base64"

# Run encode only
cargo bench --bench encoding -- "encode/"

# Run decode only
cargo bench --bench encoding -- "decode/"
```

## Understanding Output

The benchmark header shows platform info:

```
╔══════════════════════════════════════════════════════════╗
║ base-d benchmark suite                                   ║
║ Platform: x86_64 (AVX2, SSSE3)                           ║
╚══════════════════════════════════════════════════════════╝
```

Each benchmark shows throughput in MiB/s or GiB/s:

```
encode/base64/Specialized/65536
                        time:   [125.42 µs 125.84 µs 126.37 µs]
                        thrpt:  [494.59 MiB/s 496.65 MiB/s 498.31 MiB/s]
```

## Encoding Paths

The benchmarks test three SIMD paths:

| Path | Description | Availability |
|------|-------------|--------------|
| **Scalar** | Pure Rust, no SIMD | Always |
| **LUT** | Runtime lookup tables | Power-of-2 bases with SIMD support |
| **Specialized** | Hardcoded RFC tables | base64, base32, base16 on x86_64/aarch64 |

## Sample Results

### x86_64 (AVX2) - 64KB Input

| Dictionary | Scalar | LUT | Specialized |
|------------|--------|-----|-------------|
| decode_base64 | 151 MB/s | 288 MB/s | **7.36 GB/s** |
| decode_base32 | 129 MB/s | 267 MB/s | **4.81 GB/s** |
| decode_base16 | 107 MB/s | 234 MB/s | **4.14 GB/s** |
| encode_base64 | 270 MB/s | 290 MB/s | **495 MB/s** |
| decode_bioctal | 109 MB/s | **230 MB/s** | - |

### Key Findings

- **Specialized decode is dominant**: 50x speedup for base64
- **LUT provides good fallback**: 2x speedup for arbitrary alphabets
- **Radix encodings are slow**: base58/base85 use division math (~0.2 MB/s)

## HTML Reports

Criterion generates detailed HTML reports:

```bash
# After running benchmarks
open target/criterion/report/index.html
```

## Dev Tools

A Python script in `scripts/bench_summary.py` provides quick terminal summaries:

```bash
python3 scripts/bench_summary.py           # Compact table
python3 scripts/bench_summary.py --detailed # Per-dictionary breakdown
```

This is for development convenience only and not part of the distributed crate.

## Platform Detection

The benchmark module exposes platform info:

```rust
use base_d::bench::PlatformInfo;

let info = PlatformInfo::detect();
println!("{}", info.display());  // "x86_64 (AVX2, SSSE3)" or "aarch64 (NEON)"
```
