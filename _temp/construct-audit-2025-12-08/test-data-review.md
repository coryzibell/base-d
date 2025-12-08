# Test Data Review

## Summary

The base-d project takes a minimalist approach to test data - all test inputs are inlined directly in test functions. There are **no dedicated fixture files, no mock implementations, and no test data generators**. Test data is hardcoded as string literals, byte arrays, and simple iterators.

**The good news?** The edge case coverage is exceptional. The `edge_cases.rs` file is a masterclass in comprehensive testing - 539 lines of carefully thought-out boundary conditions, Unicode handling, and malformed input validation.

**The concern?** As the project grows, inline test data becomes harder to maintain, reuse, and evolve. There's no centralized place to update common test patterns when schemas change.

## Fixture Assessment

| Fixture Set | Realistic | Current | Comprehensive | Issues |
|-------------|-----------|---------|---------------|--------|
| **None exist** | N/A | N/A | N/A | All test data is inline - no fixtures |

### Test Data Location

Test data exists in three places:

1. **`src/tests.rs`** - Core encoding/decoding tests with RFC 4648 test vectors
2. **`tests/cli.rs`** - CLI integration tests with realistic command-line usage
3. **`src/encoders/algorithms/schema/edge_cases.rs`** - Comprehensive edge case suite

All data is hardcoded inline - no external files.

## Mock Assessment

| External Service | Mocked | Realistic | Error Cases | Notes |
|------------------|--------|-----------|-------------|-------|
| **None** | N/A | N/A | N/A | No external dependencies to mock |

This project has no external service dependencies - it's a pure encoding/decoding library. No mocks needed.

## Edge Case Coverage

| Category | Covered | Missing | Priority |
|----------|---------|---------|----------|
| **Strings** | ✅ Excellent | See below | Low |
| **Numbers** | ✅ Excellent | Subnormal floats | Low |
| **Collections** | ✅ Good | Very large arrays (1M+ items) | Low |
| **Dates** | ❌ None | Not applicable - no date types | N/A |
| **Null/Empty** | ✅ Excellent | - | - |
| **Unicode** | ✅ Excellent | Combining diacritics | Low |
| **Binary** | ✅ Excellent | - | - |

### Edge Cases Covered

**String Edge Cases** (from `edge_cases.rs`)
- ✅ Empty strings
- ✅ Whitespace-only strings
- ✅ Very long strings (100KB)
- ✅ Unicode emoji
- ✅ CJK characters
- ✅ RTL text (Arabic)
- ✅ Zero-width characters
- ✅ Newlines and mixed whitespace

**Numeric Edge Cases** (from `edge_cases.rs`)
- ✅ i64::MAX, i64::MIN
- ✅ u64::MAX, zero
- ✅ f64 very small (1e-308)
- ✅ f64 very large (1e308)
- ✅ f64 many decimals (pi)
- ✅ Negative zero

**Collection Edge Cases** (from `edge_cases.rs`)
- ✅ Empty objects/arrays
- ✅ Single-item arrays
- ✅ Many items (1000 rows, 100 fields)
- ✅ Deeply nested objects (50+ levels)
- ✅ Sparse arrays (missing fields)
- ✅ Heterogeneous types

**Binary Edge Cases** (from `tests.rs`)
- ✅ All 256 byte values (0..=255)
- ✅ Leading zeros preservation
- ✅ Empty input
- ✅ Single byte (zero, max)

**Encoding-Specific** (from `tests.rs`)
- ✅ RFC 4648 official test vectors (base64, base32, base16, base32hex)
- ✅ IETF Base58 test vectors
- ✅ Geohash encoding edge cases
- ✅ Compression round-trips (gzip, zstd, brotli, lz4)

This is **really** well thought out. Someone cared deeply about correctness.

## Findings

### 1. No Centralized Test Data

- **Issue:** Test data is scattered across multiple files with duplication
- **Location:** `tests/cli.rs`, `src/tests.rs`, `src/encoders/algorithms/schema/edge_cases.rs`
- **Impact:** When RFC test vectors need updating, you have to search for them. When adding a new dictionary, you have to remember to add tests in multiple places.
- **Recommendation:** Create a `test_data` module with reusable test patterns:
  ```rust
  // src/test_data.rs
  pub const RFC_VECTORS_BASE64: &[(&[u8], &str)] = &[
      (b"", ""),
      (b"f", "Zg=="),
      (b"fo", "Zm8="),
      // ...
  ];
  ```
- **Priority:** Medium

### 2. Random Data Not Seeded

- **Issue:** Benchmark random data uses a seeded PRNG (good!), but it's not exposed for tests
- **Location:** `benches/encoding.rs:43-55`
- **Impact:** Tests can't easily generate reproducible random data for stress testing
- **Recommendation:** Extract `generate_random_data()` to a shared test util:
  ```rust
  // src/test_util.rs
  pub fn generate_random_data(size: usize, seed: u64) -> Vec<u8>
  ```
- **Priority:** Low

### 3. No Property-Based Testing

- **Issue:** Tests use hardcoded examples, not generative properties
- **Location:** All test files
- **Impact:** Might miss edge cases that proptest would discover
- **Recommendation:** Add `proptest` or `quickcheck` for properties like:
  - "Encode then decode always returns original data"
  - "All 256 byte values can be encoded and decoded"
  - "Compression preserves semantics"
- **Priority:** Medium
- **Example:**
  ```rust
  proptest! {
      #[test]
      fn roundtrip_always_works(data: Vec<u8>) {
          let encoded = encode(&data, &dict);
          let decoded = decode(&encoded, &dict).unwrap();
          prop_assert_eq!(data, decoded);
      }
  }
  ```

### 4. Missing Concurrency Edge Cases

- **Issue:** No tests for concurrent access to dictionaries or encoding
- **Location:** Nowhere
- **Impact:** Might miss thread-safety issues if dictionaries are shared
- **Recommendation:** Add tests that encode/decode from multiple threads simultaneously
- **Priority:** Low (only if library supports concurrent use)

### 5. CLI Test Data Not Separated from Logic

- **Issue:** CLI tests inline all test data - no reusable examples
- **Location:** `tests/cli.rs`
- **Impact:** Hard to generate example commands for documentation
- **Recommendation:** Extract test cases to data structures:
  ```rust
  struct CliTestCase {
      name: &'static str,
      args: &'static [&'static str],
      stdin: &'static str,
      expected_stdout: &'static str,
  }
  ```
- **Priority:** Low

## Missing Edge Cases

### High Priority
- None - coverage is excellent

### Medium Priority
- **Subnormal floats** - Test f64 values near zero (f64::MIN_POSITIVE)
- **NaN/Infinity** - Test JSON handling of special float values
- **Very large arrays** - Test 1M+ item arrays for memory/performance limits

### Low Priority
- **Combining diacritics** - Unicode normalization (café vs café)
- **Surrogate pairs** - High Unicode codepoints (U+10000+)
- **Line ending variations** - CRLF vs LF in JSON strings

## Stale Fixtures

**N/A** - No fixtures exist to become stale.

## Stale Mocks

**N/A** - No mocks exist to become stale.

## Sensitive Data Audit

| Location | Type | Real Data? | Action Needed |
|----------|------|------------|---------------|
| **None found** | - | No | None |

All test data uses obvious fake values:
- `"hello world"`, `"test data 123"`
- `{"id":1,"name":"alice"}`
- No API keys, no credentials, no PII

## Data Generation Strategy

**Current approach:** Manual inline test data creation

**Randomness:** Seeded PRNG in benchmarks only (seed: `0xDEADBEEF`)

**Recommendation:**

1. **Extract common patterns** to a `test_data` module:
   ```rust
   pub mod test_data {
       pub const SMALL_INPUT: &[u8] = b"Hello, World!";
       pub const EMPTY: &[u8] = b"";
       pub const ALL_BYTES: [u8; 256] = [0, 1, 2, ..., 255];

       pub fn random_data(size: usize, seed: u64) -> Vec<u8> { ... }
       pub fn utf8_edge_cases() -> &'static [&'static str] { ... }
   }
   ```

2. **Add property-based testing** with `proptest`:
   ```toml
   [dev-dependencies]
   proptest = "1.0"
   ```

3. **Consider a test data builder** for schema tests:
   ```rust
   SchemaTestBuilder::new()
       .field("id", FieldType::U64)
       .field("name", FieldType::String)
       .row([1, "alice"])
       .row([2, "bob"])
       .build()
   ```

## Recommendations

### Critical

**None** - Test data quality is good for current project size.

### Important

**1. Add Property-Based Testing**
- Use `proptest` or `quickcheck` for generative testing
- Focus on roundtrip properties and invariants
- Catches edge cases you didn't think of

**2. Extract RFC Test Vectors**
- Create a central `test_data` module with standard test vectors
- Make them reusable across unit tests and integration tests
- Easier to update when standards change

### Nice to Have

**3. Create a Test Data Builder**
- Build complex schema test cases programmatically
- Less error-prone than hand-writing JSON strings
- Easier to maintain when schema types evolve

**4. Add Stress Tests**
- Test with 1M+ item arrays
- Test with 1GB+ strings
- Verify memory limits and error handling

**5. Document Test Data Patterns**
- Add a `TESTING.md` guide showing how to add new test cases
- Explain the test data philosophy
- Document which edge cases are intentionally not tested

## What's Good

**The `edge_cases.rs` file is a work of art.** Seriously. Someone thought deeply about:
- Unicode edge cases (emoji, CJK, RTL, zero-width)
- Numeric limits (i64 min/max, f64 extremes, negative zero)
- Structural edge cases (50-level nesting, 100-field objects, sparse arrays)
- Compression round-trips with all algorithms
- Malformed input validation

**RFC test vectors are comprehensive.** Every standard encoding (base64, base32, base16, base32hex, base58) has official test vectors from RFC 4648 and IETF drafts. This is the right way to build a standards-compliant library.

**Benchmark data is reproducible.** The seeded PRNG ensures benchmarks can be compared across runs. This is often overlooked.

**No real data in tests.** All test data is obviously fake. No risk of accidentally committing secrets or PII.

**CLI tests are realistic.** They test actual user workflows, not just the happy path. Error cases like `test_invalid_dictionary` and `test_file_not_found` show someone thought about failure modes.

## Overall Assessment

Test data quality: **B+**

The edge case coverage is A+ tier. The lack of fixtures and property-based testing brings it down to B+. For a library of this size, inline test data is acceptable - but it won't scale well as the project grows.

The biggest win would be adding `proptest` for generative testing. The second biggest would be extracting common test data to a shared module.

But honestly? The edge case coverage is so good that I'd ship this. The person who wrote `edge_cases.rs` knows what they're doing.

---

**Knock knock, Neo.**
