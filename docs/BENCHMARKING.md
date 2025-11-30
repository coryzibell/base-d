# Benchmarking Suite

## Output Format

### Full Suite Report

```
base-d benchmark suite
Platform: x86_64 (AVX2, SSSE3)
Input: 1 MB random data
Date: 2025-11-30

┌─────────────────────┬────────┬─────────────┬─────────────┬─────────────┬─────────────┐
│ Dictionary          │ Base   │ Scalar      │ LUT         │ Specialized │ Streaming   │
├─────────────────────┼────────┼─────────────┼─────────────┼─────────────┼─────────────┤
│ base64              │ 64     │ 45.2 MB/s   │ 312 MB/s    │ 1.52 GB/s   │ 1.48 GB/s   │
│ base64url           │ 64     │ 44.8 MB/s   │ 308 MB/s    │ 1.51 GB/s   │ 1.47 GB/s   │
│ base32              │ 32     │ 38.1 MB/s   │ 245 MB/s    │ 892 MB/s    │ 875 MB/s    │
│ base32hex           │ 32     │ 37.9 MB/s   │ 242 MB/s    │ 888 MB/s    │ 871 MB/s    │
│ base16              │ 16     │ 52.3 MB/s   │ 425 MB/s    │ 1.85 GB/s   │ 1.81 GB/s   │
│ hex                 │ 16     │ 51.8 MB/s   │ 418 MB/s    │ 1.82 GB/s   │ 1.78 GB/s   │
│ bioctal             │ 16     │ 50.2 MB/s   │ 412 MB/s    │ -           │ -           │
│ base256_matrix      │ 256    │ 28.5 MB/s   │ 185 MB/s    │ 725 MB/s    │ 712 MB/s    │
│ base58              │ 58     │ 12.4 MB/s   │ -           │ -           │ -           │
│ base85              │ 85     │ 18.2 MB/s   │ -           │ -           │ -           │
│ cards               │ 52     │ 15.1 MB/s   │ -           │ -           │ -           │
│ hieroglyphs         │ 100    │ 8.3 MB/s    │ -           │ -           │ -           │
│ emoji_faces         │ 80     │ 9.1 MB/s    │ -           │ -           │ -           │
└─────────────────────┴────────┴─────────────┴─────────────┴─────────────┴─────────────┘

Legend:
  - = Not supported for this dictionary/platform
  Throughput measured as input bytes processed per second
```

### Single Dictionary Report

```
base-d benchmark: base64
Platform: x86_64 (AVX2, SSSE3)

Encode (1 MB random data, 100 iterations):
┌─────────────┬────────────┬────────────┬────────────┬────────────┐
│ Path        │ Mean       │ Std Dev    │ Throughput │ vs Scalar  │
├─────────────┼────────────┼────────────┼────────────┼────────────┤
│ Scalar      │ 22.14 ms   │ ±0.31 ms   │ 45.2 MB/s  │ 1.00x      │
│ LUT         │ 3.21 ms    │ ±0.08 ms   │ 312 MB/s   │ 6.90x      │
│ Specialized │ 0.66 ms    │ ±0.02 ms   │ 1.52 GB/s  │ 33.5x      │
│ Streaming   │ 0.68 ms    │ ±0.03 ms   │ 1.48 GB/s  │ 32.6x      │
└─────────────┴────────────┴────────────┴────────────┴────────────┘

Decode (1 MB encoded data, 100 iterations):
┌─────────────┬────────────┬────────────┬────────────┬────────────┐
│ Path        │ Mean       │ Std Dev    │ Throughput │ vs Scalar  │
├─────────────┼────────────┼────────────┼────────────┼────────────┤
│ Scalar      │ 25.82 ms   │ ±0.42 ms   │ 38.7 MB/s  │ 1.00x      │
│ LUT         │ 3.85 ms    │ ±0.11 ms   │ 260 MB/s   │ 6.71x      │
│ Specialized │ 0.78 ms    │ ±0.03 ms   │ 1.28 GB/s  │ 33.1x      │
│ Streaming   │ 0.81 ms    │ ±0.04 ms   │ 1.23 GB/s  │ 31.9x      │
└─────────────┴────────────┴────────────┴────────────┴────────────┘
```

### Size Scaling Report

```
base-d benchmark: base64 (size scaling)
Platform: x86_64 (AVX2)
Path: Specialized

┌────────────┬────────────┬────────────┬────────────┐
│ Input Size │ Encode     │ Decode     │ Throughput │
├────────────┼────────────┼────────────┼────────────┤
│ 16 B       │ 42 ns      │ 51 ns      │ 380 MB/s   │
│ 256 B      │ 118 ns     │ 142 ns     │ 2.17 GB/s  │
│ 1 KB       │ 312 ns     │ 385 ns     │ 3.20 GB/s  │
│ 16 KB      │ 4.2 µs     │ 5.1 µs     │ 3.81 GB/s  │
│ 1 MB       │ 0.66 ms    │ 0.78 ms    │ 1.52 GB/s  │
│ 100 MB     │ 68.2 ms    │ 81.5 ms    │ 1.47 GB/s  │
└────────────┴────────────┴────────────┴────────────┘
```

## CLI Interface

```bash
# Full suite with random data
base-d bench

# Full suite with specific size
base-d bench --size 1mb

# Single dictionary
base-d bench --dict base64

# Custom input file
base-d bench --input data.bin

# Specific paths only
base-d bench --dict base64 --paths scalar,specialized

# JSON output for CI
base-d bench --json > benchmark.json

# Quick mode (fewer iterations)
base-d bench --quick

# Size scaling test
base-d bench --dict base64 --scaling
```

## Implementation

### Path Detection

For each dictionary, detect available paths:

```rust
struct BenchmarkPaths {
    scalar: bool,      // Always true
    lut: bool,         // Power-of-2 base, ASCII chars
    specialized: bool, // Known RFC dictionary + platform support
    streaming: bool,   // Chunked mode dictionaries
}

fn detect_paths(dict: &Dictionary, platform: Platform) -> BenchmarkPaths {
    // ...
}
```

### Platform Detection

```rust
enum Platform {
    X86_64 { avx2: bool, ssse3: bool, avx512: bool },
    Aarch64 { neon: bool },
    Other,
}
```

### Benchmark Runner

```rust
struct BenchmarkResult {
    dictionary: String,
    path: String,
    operation: Operation, // Encode | Decode
    input_size: usize,
    iterations: usize,
    mean_ns: u64,
    std_dev_ns: u64,
    throughput_mbps: f64,
}
```
