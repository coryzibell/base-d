# SIMD Optimizations

## Overview

base-d includes **SIMD (Single Instruction, Multiple Data) optimizations** for base64 encoding on x86_64 platforms with SSSE3 support. These optimizations provide automatic performance improvements with **zero configuration required**.

## How It Works

### Automatic Detection

base-d uses **runtime CPU feature detection** with cached results for zero-overhead dispatch:

```rust
// First call: Detects CPU features (~1ns overhead)
let encoded = encode(data, &base64_dictionary);

// Subsequent calls: Zero overhead (cached detection)
let encoded2 = encode(more_data, &base64_dictionary);
```

### Fallback Strategy

If SIMD is not available or not applicable, base-d automatically falls back to the highly-optimized scalar implementation:

```
┌─────────────────────────────┐
│ encode(data, dictionary)    │
└──────────┬──────────────────┘
           │
           ├─ Is x86_64? ──No──> Scalar implementation
           │       │
           │      Yes
           │       │
           ├─ Has AVX2/SSSE3? ──No──> Scalar implementation
           │       │
           │      Yes
           │       │
           ├─ Is base64? ──No──> Scalar implementation
           │       │
           │      Yes
           │       │
           └──> SIMD-accelerated encoding 🚀
```

## Performance

### Current Status

**SIMD Encoding:** ✅ Complete
- Runtime SSSE3 CPU detection with caching
- Automatic fallback to scalar code
- Zero-overhead abstraction
- Based on [aklomp/base64](https://github.com/aklomp/base64) algorithm

**SIMD Decoding:** 🚧 Planned
- Currently uses optimized scalar implementation
- SIMD decoding planned for future release

### Expected Performance

| Operation | Scalar | SIMD (SSSE3) | Notes |
|-----------|--------|--------------|-------|
| Base64 Encode | ~370 MiB/s | **~1.5 GiB/s** | 4x improvement |
| Base64 Decode | ~220 MiB/s | Scalar | SIMD pending |

*Run `cargo bench` to measure on your hardware*

## Supported Platforms

### x86_64 (Intel/AMD)

| Feature Level | Status | Performance Gain |
|---------------|--------|------------------|
| **SSSE3** | ✅ Complete | ~4x |
| **AVX2** | ⏳ Planned | ~6-8x expected |
| **AVX-512** | 📋 Future | ~10x+ expected |
| Scalar fallback | ✅ Always available | Baseline |

### Other Architectures

| Architecture | Status | Notes |
|-------------|--------|-------|
| **ARM (NEON)** | ⏳ Planned | v0.3.0 target |
| **WASM (SIMD128)** | ⏳ Planned | v0.4.0 target |
| **RISC-V** | 📋 Future | After v1.0 |

## Technical Details

### SSSE3 Implementation

**Processing Model:**
- Input: 12 bytes → Output: 16 base64 characters per iteration
- Uses 128-bit XMM registers
- Remainder handled by optimized scalar code

**Key Techniques:**
1. **Byte shuffling (`PSHUFB`)** - Rearrange bytes to prepare for 6-bit extraction
2. **Multiply tricks** - Use `PMULHUW`/`PMULLW` to shift bits into place (avoids slow variable shifts)
3. **Offset-based lookup** - Convert 6-bit indices to ASCII with a single shuffle + add

**Algorithm (from aklomp/base64):**
```
enc_reshuffle:
1. Shuffle: Duplicate bytes for each 3→4 expansion
2. AND + MULHI: Extract bits for positions 0,2
3. AND + MULLO: Extract bits for positions 1,3
4. OR: Combine results → 16 x 6-bit indices

enc_translate:
1. Compute lookup index from 6-bit value
2. Shuffle to get ASCII offset
3. Add offset to value → ASCII character
```

**Reference Implementation:**
Based on techniques from:
- [aklomp/base64](https://github.com/aklomp/base64) - High-performance C implementation
- [Wojciech Muła's SIMD work](http://0x80.pl/notesen/2016-01-12-sse-base64-encoding.html)
- Intel optimization manuals

### CPU Feature Detection

**First Call (Cold Path):**
```rust
static HAS_AVX2: OnceLock<bool> = OnceLock::new();

pub fn has_avx2() -> bool {
    *HAS_AVX2.get_or_init(|| {
        is_x86_feature_detected!("avx2")  // ~1ns
    })
}
```

**Subsequent Calls (Hot Path):**
```rust
pub fn has_avx2() -> bool {
    *HAS_AVX2.get()  // Just a pointer dereference (~0.3ns)
}
```

### Limitations

**Current SIMD optimizations only apply to:**
- ✅ x86_64 architecture
- ✅ Standard base64 encoding (RFC 4648)
- ✅ Dictionary: `A-Za-z0-9+/`

**NOT optimized (uses scalar):**
- ❌ base32, base58, custom dictionaries
- ❌ Base64url (different dictionary)
- ❌ Mathematical base conversion mode
- ❌ Byte range mode

## Benchmarking SIMD

### Run Benchmarks

```bash
# All benchmarks (includes base64 SIMD)
cargo bench

# Just base64 benchmarks
cargo bench --bench encoding base64

# Save baseline for comparison
cargo bench -- --save-baseline before-simd

# Compare after changes
cargo bench -- --baseline before-simd
```

### Check CPU Features

```bash
# On Linux
cat /proc/cpuinfo | grep flags

# On Windows (PowerShell)
Get-WmiObject Win32_Processor | Select-Object -ExpandProperty Name

# In Rust (at runtime)
if is_x86_feature_detected!("avx2") {
    println!("AVX2 available!");
}
```

## Development Roadmap

### Phase 1: Infrastructure ✅
- [x] Runtime CPU detection with caching
- [x] SIMD module structure
- [x] Automatic fallback mechanism
- [x] Integration with chunked encoding
- [x] Test framework

### Phase 2: SSSE3 Base64 Encoding ✅
- [x] Byte shuffling (enc_reshuffle)
- [x] 6-bit extraction via multiply
- [x] Offset-based ASCII lookup (enc_translate)
- [x] Remainder handling with scalar
- [x] Comprehensive tests

### Phase 3: Decoding & More Encodings ⏳
- [ ] Base64 decoding with SIMD
- [ ] Base64url support (different +/ chars)
- [ ] Hex SIMD optimization (4-bit)
- [ ] Base32 SIMD optimization (5-bit)

### Phase 4: AVX2 Optimization ⏳
- [ ] 24-byte blocks (256-bit registers)
- [ ] Further performance gains

### Phase 5: ARM Support (v0.3.0)
- [ ] NEON intrinsics for ARM64
- [ ] Runtime feature detection for ARM
- [ ] Cross-platform testing

### Phase 6: Advanced Features (v0.4.0)
- [ ] AVX-512 support
- [ ] WASM SIMD128
- [ ] Parallel processing for large files (rayon)

## Debugging

### Verify SIMD Usage

```rust
use base_d::simd::has_avx2;

if has_avx2() {
    println!("AVX2 enabled - SIMD optimizations active!");
} else {
    println!("No AVX2 - using optimized scalar code");
}
```

### Performance Testing

```bash
# Run with perf (Linux)
perf stat -d cargo bench --bench encoding base64

# Check SIMD instruction usage
perf record -e cycles,instructions cargo bench
perf report

# Profile with VTune (Intel)
vtune -collect hotspots -- cargo bench
```

### Known Limitations

1. **Decoding**: SIMD decoding is not yet implemented (returns `None`, uses scalar)
2. **Only Standard Base64**: Custom dictionaries and base64url don't use SIMD yet
3. **Minimum input size**: Inputs < 16 bytes use scalar code

## Contributing

Want to help extend SIMD support? Check out:

1. **Reference implementations:**
   - https://github.com/aklomp/base64
   - http://0x80.pl/notesen/2016-01-12-sse-base64-encoding.html

2. **Key files:**
   - `src/simd/mod.rs` - CPU feature detection
   - `src/simd/x86_64.rs` - SSSE3 encoding implementation
   - `enc_reshuffle()` - Byte shuffling and bit extraction
   - `enc_translate()` - Index to ASCII conversion

3. **Testing:**
   - `cargo test simd` - Run SIMD tests
   - `cargo bench` - Benchmark against scalar

## References

- [Intel Intrinsics Guide](https://www.intel.com/content/www/us/en/docs/intrinsics-guide/index.html)
- [Agner Fog's Optimization Manuals](https://www.agner.org/optimize/)
- [Base64 SIMD Encoding (Wojciech Muła)](http://0x80.pl/notesen/2016-01-12-sse-base64-encoding.html)
- [Fast Base64 Encoding (aklomp)](https://github.com/aklomp/base64)
- [Rust SIMD Support](https://doc.rust-lang.org/std/arch/)
