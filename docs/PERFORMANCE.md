# Performance Optimization Report

## Summary

Implemented performance optimizations for base-d encoding/decoding operations with focus on:
1. **Memory allocation efficiency** - Pre-allocate buffers with correct capacity
2. **CPU cache optimization** - Process data in chunks for better cache utilization  
3. **Fast lookup tables** - Array-based lookups for ASCII dictionaries instead of HashMap
4. **Algorithmic improvements** - Use `div_rem` instead of separate division and modulo operations

## Optimizations Applied

### 1. Chunked Encoding (base64/base32) - `src/chunked.rs`
- **Pre-allocated output buffers** with calculated capacity
- **Chunk-based processing** (64 bytes per chunk) for better CPU cache utilization
- **Results**: Base64 encoding at ~370 MiB/s, decoding at ~220 MiB/s

### 2. Byte Range Encoding (base100) - `src/byte_range.rs`
- **Pre-allocated String** with exact capacity (data.len() * 4 bytes)
- **Chunk-based processing** (64 bytes per chunk)
- **Pre-collect chars** for decode to avoid repeated iteration overhead

### 3. Mathematical Base Conversion - `src/encoding.rs`
- **Pre-allocated result vector** with estimated capacity
- **Combined div_rem** operation instead of separate % and / operations
- **Vec::resize** instead of loop for leading zeros
- **Pre-collect chars** for decode path

### 4. Fast Dictionary Lookups - `src/dictionary.rs`
- **Added lookup table** for ASCII characters (Box<[Option<usize>; 256]>)
- **O(1) array access** for ASCII chars instead of O(log n) HashMap lookup
- **Fallback to HashMap** for non-ASCII characters
- **Automatic detection** - table built only when all chars are ASCII

## Benchmark Results

### Base64 Encoding (Chunked Mode)
| Size | Throughput | Time |
|------|-----------|------|
| 64 B | 297 MiB/s | 206 ns |
| 256 B | 335 MiB/s | 729 ns |
| 1 KB | 356 MiB/s | 2.74 µs |
| 4 KB | 367 MiB/s | 10.6 µs |
| 16 KB | 370 MiB/s | 42.3 µs |

### Base64 Decoding (Chunked Mode)
| Size | Throughput | Time |
|------|-----------|------|
| 64 B | 133 MiB/s | 458 ns |
| 256 B | 193 MiB/s | 1.26 µs |
| 1 KB | 214 MiB/s | 4.56 µs |
| 4 KB | 220 MiB/s | 17.8 µs |
| 16 KB | 219 MiB/s | 71.2 µs |

### Base32 Encoding (Chunked Mode)
| Size | Throughput | Time |
|------|-----------|------|
| 64 B | 278 MiB/s | 220 ns |
| 256 B | 215 MiB/s | 1.14 µs |
| 1 KB | 270 MiB/s | 3.62 µs |

## Performance Characteristics

### CPU Cache Optimization
Chunk size of 64 bytes chosen because:
- Fits comfortably in L1 cache (typically 32-64 KB)
- Aligns with common cache line size (64 bytes)
- Reduces branch mispredictions in tight loops

### Memory Allocation
Pre-allocation reduces:
- Reallocation overhead (typically 2x growth)
- Memory fragmentation
- CPU cycles spent in allocator

### Lookup Table vs HashMap
For ASCII dictionaries (base64, base32, hex):
- Array lookup: ~1-2 ns (direct memory access)
- HashMap lookup: ~5-10 ns (hash calculation + probe)
- **Result**: ~5x faster character decoding

For large non-ASCII dictionaries (base1024):
- Uses HashMap for all characters (no lookup table)
- Still benefits from pre-allocation and chunking optimizations
- Encode: ~7 MiB/s, Decode: ~21 MiB/s (64-byte blocks)
- Mathematical mode with large dictionaries is computationally intensive due to BigUint operations

## Future Optimization Opportunities

### SIMD (Single Instruction Multiple Data)
Potential for 4-8x speedup in:
1. **Base64 encoding/decoding** - Process 16-32 bytes at once
   - Use `std::arch::x86_64` intrinsics
   - Lookup tables with SIMD shuffle instructions
   - Already implemented in some Rust base64 crates (e.g., `base64-simd`)

2. **Byte range operations** - Parallel codepoint arithmetic
   - SIMD add/subtract for byte-to-codepoint mapping
   - Vectorized bounds checking

### Platform-Specific Optimizations
1. **x86_64**: AVX2/AVX-512 for 256/512-bit SIMD
2. **ARM**: NEON SIMD instructions
3. **WASM**: SIMD128 when `simd128` feature is stable

### Parallel Processing
For large files (>1 MB):
- Use `rayon` for parallel chunk processing
- Independent encoding of multiple chunks
- Requires careful coordination for mathematical mode

## Dependencies Added

```toml
[dependencies]
num-integer = "0.1"  # For Integer::div_rem trait

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
```

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench encoding

# Generate HTML reports
cargo bench -- --save-baseline baseline-name

# Compare against baseline
cargo bench -- --baseline baseline-name
```

## Next Steps

1. **Profile with perf/VTune** to identify remaining bottlenecks
2. **Implement SIMD for base64** (highest impact, most common use case)
3. **Add benchmark for base100** byte range mode
4. **Optimize BigUint operations** for mathematical mode
5. **Consider lookup tables** for other hot paths

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [SIMD for base64](https://github.com/marshallpierce/rust-base64)
