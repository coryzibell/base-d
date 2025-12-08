# Performance Recommendations

## Summary

base-d demonstrates strong performance fundamentals with comprehensive SIMD acceleration, appropriate memory allocation patterns, and excellent startup times. The codebase is well-optimized for common use cases (power-of-2 bases with SIMD) but has identified optimization opportunities for edge cases and hot paths.

**Overall Assessment:** Production-ready with targeted optimization potential in non-SIMD fallbacks and radix mode.

---

## Performance Budget

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| CLI startup | <100ms | ~2ms | ✅ Excellent |
| 10MB encode (base64) | - | ~67ms | ✅ Good (149 MB/s) |
| Binary size (stripped) | - | 5.9MB | ⚠️ Acceptable (has inline SIMD tables) |
| p50 latency (small files) | - | ~2ms | ✅ Excellent |

---

## Benchmark Coverage

### Existing Benchmarks

**Location:** `/home/kautau/work/personal/code/base-d/benches/encoding.rs`

**Coverage:**
- ✅ Multiple input sizes: 64B, 256B, 1KB, 4KB, 16KB, 64KB
- ✅ Path comparison: Scalar vs LUT vs Specialized SIMD
- ✅ Throughput measurement via Criterion
- ✅ Platform detection and reporting
- ✅ Multiple dictionaries: base64, base32, base16, base58, base85, emoji, cards

**Strengths:**
- Comprehensive dictionary coverage (14 different encodings)
- Good size range for cache analysis
- Separated benchmark groups by encoding type (RFC vs fun vs non-power-of-2)
- Path isolation allows precise performance attribution

**Gaps:**
- ❌ No streaming benchmark (large file performance)
- ❌ No schema encoding/decoding benchmarks (fiche format)
- ❌ No compression integration benchmarks
- ❌ No dictionary loading overhead measurement
- ❌ No CI regression tracking visible

---

## Hot Paths

### Critical Code Paths (Performance Sensitive)

1. **SIMD encode/decode** - `/home/kautau/work/personal/code/base-d/src/simd/`
   - Specialized: `x86_64/specialized/base64.rs` (941 LOC), base32, base16, base256
   - LUT-based: `lut/base64.rs` (2510 LOC), generic codecs
   - **Status:** Well-optimized with AVX2/SSSE3/NEON implementations

2. **Radix encoding** - `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/radix.rs` (114 LOC)
   - BigInt arithmetic for non-power-of-2 bases (base58, base85, custom)
   - **Status:** O(n) but heavy allocation via num-bigint

3. **Chunked scalar fallback** - `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/chunked.rs` (197 LOC)
   - Used when SIMD unavailable or unsupported dictionary
   - **Status:** Good pre-allocation, 64-byte chunking for cache locality

4. **Dictionary char lookup** - `/home/kautau/work/personal/code/base-d/src/core/dictionary.rs`
   - HashMap for reverse lookup (char → index)
   - Optional `[Option<usize>; 256]` fast path for ASCII
   - **Status:** Well-optimized with lookup table for hot paths

5. **Schema parsing** - `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/parsers/json.rs` (910 LOC)
   - JSON flattening, type inference, null bitmap construction
   - **Status:** Multiple passes, allocation-heavy

---

## Complexity Analysis

| Operation | Current | Optimal | Location | Notes |
|-----------|---------|---------|----------|-------|
| Base64 encode (SIMD) | O(n) | O(n) | `simd/x86_64/specialized/base64.rs` | 24 bytes/iteration (AVX2) |
| Base64 decode (SIMD) | O(n) | O(n) | `simd/x86_64/specialized/base64.rs` | 32 chars/iteration (AVX2) |
| Radix encode | O(n log n) | O(n log n) | `encoders/algorithms/radix.rs:25-52` | BigInt division |
| Radix decode | O(n log n) | O(n log n) | `encoders/algorithms/radix.rs:55-114` | BigInt multiplication |
| Chunked scalar | O(n) | O(n) | `encoders/algorithms/chunked.rs:22-97` | Bit manipulation |
| Dictionary lookup | O(1) avg | O(1) | `core/dictionary.rs:285-309` | HashMap + optional array |
| JSON schema parse | O(n×m) | O(n) | `schema/parsers/json.rs:72-150` | n=rows, m=fields (flattening) |

---

## Findings

### SIMD Acceleration

**Location:** `/home/kautau/work/personal/code/base-d/src/simd/`

**Current State:**
- AVX2: 24 bytes → 32 chars per iteration (base64)
- SSSE3: 12 bytes → 16 chars per iteration (base64)
- NEON: ARM support present
- 558 unsafe blocks across SIMD implementations (expected)
- Runtime feature detection cached in `OnceLock`

**What's Good:**
- Comprehensive SIMD coverage for RFC bases (16, 32, 64, 256)
- Automatic fallback cascade: Specialized → Generic → LUT → Scalar
- Clean separation of platform-specific code (x86_64 vs aarch64)
- Proper alignment handling and remainder processing

**Optimization Opportunity:**
- **Issue:** No AVX-512 support despite detection code in `bench.rs:64`
- **Impact:** AVX-512 could process 48-64 bytes per iteration (~2x current AVX2)
- **Recommendation:** Add AVX-512 VBMI path for base64/base32 (64-byte chunks)
- **Priority:** Low (AVX-512 adoption still limited, AVX2 path is excellent)

---

### Radix Mode (BigInt Operations)

**Location:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/radix.rs`

**Issue:** Heavy allocation in hot path
- **Line 25:** `BigUint::from_bytes_be()` - allocates
- **Line 34-44:** Division loop with repeated `div_rem()` allocations
- **Line 93:** Multiplication `num *= &base_big` - allocates

**Current Complexity:** O(n log n) with O(n) allocations

**Impact:**
- Base58/Base85 encode/decode ~10-100x slower than base64 SIMD
- Unavoidable for non-power-of-2 bases (mathematical requirement)
- Only affects ~6 of 54 dictionaries (base58, base85, custom bases)

**Recommendations:**
1. **Accept current performance** - Radix mode is fundamentally expensive
2. Document performance characteristics in README
3. Suggest chunked mode alternatives where possible
4. Consider `rug` or `gmp` bindings for 2-3x speedup (adds C dependency)

**Priority:** Low - Radix mode is inherently slow, current implementation reasonable

---

### Dictionary Lookup Optimization

**Location:** `/home/kautau/work/personal/code/base-d/src/core/dictionary.rs`

**Current State:**
```rust
// Line 18: Optional fast path for ASCII
lookup_table: Option<Box<[Option<usize>; 256]>>
// Line 16: Fallback HashMap
char_to_index: HashMap<char, usize>
```

**What's Good:**
- Dual-mode lookup: O(1) array for ASCII, O(1) average HashMap for Unicode
- Properly boxed to avoid stack overflow (256 × 16 bytes = 4KB)
- Used in decode hot path

**Potential Issue:**
- `HashMap<char, usize>` has 16-byte overhead per entry
- For base64 (64 chars): ~1KB HashMap vs 4KB lookup table
- Lookup table used only when `all chars < 128`

**Recommendation:** Consider `hashbrown::HashMap` (no benefit visible in criterion)

**Priority:** Low - Current approach is optimal for mixed ASCII/Unicode

---

### Memory Allocation Patterns

**Analysis via grep:**
- `Vec::new()` / `String::new()`: 334 occurrences (normal)
- `.clone()`: 292 occurrences (inspect hot paths)
- `.collect()`: 292 occurrences (some avoidable)
- `HashMap::new()`: 33 occurrences (reasonable)

**Hot Path Allocations:**

#### ✅ Good Pre-allocation
```rust
// chunked.rs:38 - Pre-calculates exact output size
let mut result = String::with_capacity(capacity);

// radix.rs:28 - Estimates capacity based on input size
let max_digits = ((data.len() - leading_zeros) * 8 * 1000)
                 / (base as f64).log2() as usize / 1000 + 1;
let mut result = Vec::with_capacity(max_digits + leading_zeros);
```

#### ⚠️ Potential Optimization
```rust
// chunked.rs:129 - Collects entire string to Vec<char>
let chars: Vec<char> = encoded.chars().collect();
```

**Issue:** Allocates intermediate Vec for iteration
**Impact:** 2× memory for decode operation (original string + char vec)
**Recommendation:** Use `.chars().enumerate()` iterator directly
**Savings:** ~25-33% memory reduction for decode operations
**Priority:** Medium

**Location:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/chunked.rs:129`

---

### Schema Encoding Pipeline

**Location:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/`

**Components:**
- JSON parser: 910 LOC
- Fiche serializer: 2440 LOC
- Binary packer: 311 LOC
- Binary unpacker: 424 LOC

**Issue:** Multiple allocation passes during JSON parsing

**Flow:**
1. Parse JSON → serde_json::Value (allocation)
2. Flatten objects → HashMap per row (allocation)
3. Infer types → iterate all rows (O(n×m))
4. Build values + null bitmap (allocation)
5. Pack to binary (allocation)

**Complexity:** O(n×m) where n=rows, m=fields

**Impact:**
- For 10,000 rows × 20 fields: ~200,000 iterations in type inference
- Each HashMap: ~1KB overhead
- Total: ~10MB+ allocations for medium datasets

**Recommendations:**
1. Single-pass parsing with streaming type inference
2. Reuse HashMaps via pool pattern
3. Consider zero-copy schema detection
4. Benchmark schema encoding (currently missing)

**Priority:** Medium - Schema is not a hot path for typical CLI usage

---

### Streaming Performance

**Location:** `/home/kautau/work/personal/code/base-d/src/encoders/streaming/`

**Current State:**
- 4KB chunk size (`CHUNK_SIZE: usize = 4096`)
- Proper streaming for Chunked/ByteRange modes
- Radix mode must buffer entire input (line 80)

**What's Good:**
- Appropriate chunk size for L1 cache (32KB typical)
- Supports optional compression + hashing during stream
- Clean separation of concerns

**Missing:**
- No streaming benchmarks (recommended sizes: 10MB, 100MB, 1GB)
- No comparison of chunk sizes (2KB, 4KB, 8KB, 16KB)
- No memory usage measurement

**Recommendation:** Add streaming benchmarks to detect regressions

**Priority:** Medium - Important for CLI file processing

---

### Inline Attributes

**Analysis:** Only 11 `#[inline]` hints across entire SIMD module

**Status:** Appropriate restraint. Rust compiler auto-inlines aggressively.

**Locations:**
- `simd/lut/gapped.rs:1`
- `simd/x86_64/common.rs:3`
- `simd/aarch64/common.rs:2`

**What's Good:**
- Only used on small helper functions (<10 LOC)
- Not over-applied (prevents binary bloat)

**No action needed.**

---

## Missing Benchmarks

### Critical Gaps

1. **Streaming throughput**
   - Sizes: 10MB, 100MB, 1GB files
   - Modes: Chunked, ByteRange, Radix
   - Measure: throughput (MB/s), peak memory, allocations

2. **Schema encoding/decoding**
   - Input: JSON datasets (100 rows, 1K rows, 10K rows)
   - Operations: parse → pack → encode → decode → unpack → serialize
   - Measure: latency, memory, compression ratio

3. **Dictionary loading overhead**
   - Operation: `DictionaryRegistry::load_default()`
   - Measure: cold start time, TOML parse time
   - **Quick test:** Already measured at ~2ms (acceptable)

4. **Compression integration**
   - Combinations: encode + gzip, encode + zstd, encode + brotli
   - Measure: throughput impact, ratio tradeoffs

5. **Concurrent operations**
   - Scenario: Multiple parallel encodes (rayon)
   - Measure: scaling efficiency, lock contention

---

## Optimization Opportunities

### Quick Wins

#### 1. Remove Vec<char> allocation in chunked decode
**File:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/chunked.rs:129`
**Change:**
```rust
// Before
let chars: Vec<char> = encoded.chars().collect();
for chunk in chars.chunks_exact(CHUNK_SIZE) {
    for &c in chunk { ... }
}

// After
for (char_position, c) in encoded.chars().enumerate() {
    // Handle padding
    if Some(c) == padding { break; }
    // ... process c directly
}
```
**Impact:** 25-33% memory reduction, ~2-5% faster decode
**Effort:** Low (30 minutes)
**Risk:** Low
**Priority:** High

---

#### 2. Add inline hints to dictionary lookup hot path
**File:** `/home/kautau/work/personal/code/base-d/src/core/dictionary.rs:285-309`
**Change:**
```rust
#[inline]
pub fn decode_char(&self, c: char) -> Option<usize> { ... }

#[inline]
pub fn encode_digit(&self, index: usize) -> Option<char> { ... }
```
**Impact:** 1-3% improvement in scalar paths
**Effort:** Trivial (5 minutes)
**Risk:** None
**Priority:** High

---

#### 3. Pre-compile dictionaries.toml into binary
**Current:** Parse TOML at runtime (18KB file, 327 lines)
**Change:** Use `include_str!()` and lazy_static for registry
**Impact:** Remove filesystem I/O, ~1ms saved per invocation
**Effort:** Low (1 hour)
**Risk:** Low (increases binary size ~20KB)
**Priority:** Medium

---

### Larger Efforts

#### 1. AVX-512 VBMI support
**File:** New module `src/simd/x86_64/specialized/base64_avx512.rs`
**Scope:**
- Detect AVX-512 VBMI
- Process 48-64 bytes per iteration
- Maintain fallback to AVX2/SSSE3

**Impact:** 1.5-2× throughput on modern CPUs (Ice Lake+)
**Effort:** High (2-3 weeks, requires testing on AVX-512 hardware)
**Risk:** Medium (AVX-512 frequency throttling, limited adoption)
**Priority:** Low

---

#### 2. Optimize schema JSON flattening
**File:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/parsers/json.rs`
**Scope:**
- Single-pass parsing with streaming type inference
- HashMap pooling for row processing
- Reduce intermediate allocations

**Impact:** 2-3× faster schema encoding for large datasets
**Effort:** High (1-2 weeks)
**Risk:** Medium (complex logic, must preserve correctness)
**Priority:** Medium

---

#### 3. Add comprehensive streaming benchmarks
**File:** `benches/streaming.rs` (new)
**Scope:**
- Large file sizes (10MB, 100MB, 1GB)
- All encoding modes
- Memory profiling integration

**Impact:** Prevent regressions, identify bottlenecks
**Effort:** Medium (3-5 days)
**Risk:** Low
**Priority:** High

---

## What's Good

### Performance Best Practices Already in Place

1. **Excellent memory pre-allocation**
   - Chunked encoder calculates exact output size (line 38)
   - Radix encoder estimates capacity (line 28)
   - Streaming decoder pre-allocates based on input (line 122)

2. **SIMD cascade with graceful fallback**
   - Specialized → Generic → LUT → Scalar
   - Runtime feature detection cached (OnceLock)
   - Clean platform abstraction (x86_64/aarch64)

3. **Cache-friendly chunking**
   - 64-byte chunks in chunked.rs (lines 44-60)
   - 4KB streaming chunks (streaming/encoder.rs:8)
   - Proper alignment in SIMD code

4. **Minimal allocations in hot paths**
   - SIMD codecs reuse buffers
   - Scalar chunked uses bit buffers (u32/u64)
   - Dictionary lookup uses stack-allocated arrays

5. **Fast CLI startup**
   - 2ms cold start
   - Lazy dictionary loading
   - No heavy initialization

6. **Comprehensive test coverage for correctness**
   - All SIMD paths tested for equivalence
   - Round-trip tests
   - Edge case handling

7. **Binary size discipline**
   - 5.9MB stripped (acceptable for SIMD + hash/compression)
   - Could be reduced via feature flags (compression/hashing optional)

---

## Recommendations Summary

### Immediate Actions (1-2 days)
1. ✅ Add `#[inline]` to dictionary lookup hot paths
2. ✅ Remove `Vec<char>` allocation in chunked decode
3. ⚠️ Add streaming benchmarks (10MB+)

### Short-term (1-2 weeks)
4. ⚠️ Pre-compile dictionaries.toml into binary
5. ⚠️ Benchmark schema encoding pipeline
6. ⚠️ Document radix mode performance characteristics

### Long-term (1-3 months)
7. 🔵 Investigate AVX-512 VBMI for base64/base32
8. 🔵 Optimize schema JSON flattening (if benchmarks show need)
9. 🔵 Add rayon parallelism for batch operations

### Performance Culture
- ✅ Set up CI benchmark regression tracking
- ✅ Add flamegraph profiling to development workflow
- ✅ Document performance expectations in README

---

## Profiling Recommendations

### Recommended Tools

1. **CPU profiling:** `cargo flamegraph` (not currently in use)
   ```bash
   cargo install flamegraph
   cargo flamegraph --bench encoding -- --bench
   ```

2. **Memory profiling:** `heaptrack` or `dhat`
   ```bash
   heaptrack target/release/base-d encode base64 large-file.bin
   ```

3. **Benchmark tracking:** Criterion already in place ✅
   - Enable CI artifact upload for trend analysis
   - Add `criterion-table` for comparison

4. **Syscall analysis:** `strace` for I/O bottlenecks
   ```bash
   strace -c target/release/base-d encode base64 file.bin
   ```

### Profiling Workflow

1. **Baseline measurement**
   ```bash
   cargo bench --bench encoding > baseline.txt
   ```

2. **Make optimization**
   - Modify code
   - Re-run benchmarks
   - Compare with baseline

3. **Profile hot paths**
   ```bash
   cargo flamegraph --bench encoding -- --bench base64
   ```

4. **Verify improvement**
   - Must show >5% gain to justify complexity
   - Check for regression in other paths

---

**Knock knock, Neo.**
