# Error Handling

## Summary

base-d demonstrates **strong error handling fundamentals** with well-typed errors, helpful messages, and appropriate error propagation throughout most of the codebase (~33k lines). The project implements excellent user-facing error formatting with context and suggestions. However, there are scattered areas with unsafe practices that could cause unexpected runtime failures.

**Maturity Level:** Between **Informative** and **Recoverable**
- Errors carry rich context with actionable suggestions
- Custom error types with helpful Display implementations
- Some recovery paths (file size checks with --force flag)
- Missing: Comprehensive logging/observability, retry logic

## Patterns Found

- **Error type strategy:** Typed with custom enums (`DecodeError`, `SchemaError`, `DictionaryNotFoundError`)
- **Logging approach:** Basic stderr output (eprintln!), no structured logging framework
- **Recovery strategy:** Mostly fail-fast with some graceful degradation (file size limits)
- **Observability:** None - no metrics, tracing, or alerting

## Error Flow Analysis

Error propagation follows Rust best practices with `Result<T, E>` and `?` operator throughout the codebase:

1. **Library Layer** (`src/encoders/`, `src/features/`)
   - Returns typed errors: `DecodeError`, `SchemaError`
   - Uses `Box<dyn std::error::Error>` for heterogeneous errors in compression/hashing
   - Proper error context with `Result` types (168 occurrences across 36 files)

2. **CLI Layer** (`src/cli/handlers/`)
   - All handlers return `Result<(), Box<dyn std::error::Error>>`
   - Errors bubble up cleanly to `main.rs`
   - User-friendly messages printed to stderr before exit

3. **Main Entry Point** (`src/main.rs`)
   - Simple error handling: catches, prints, exits with code 1
   - No differentiation of exit codes based on error type

Error context is added appropriately as errors propagate up the stack, with high-quality Display implementations that include:
- Colored terminal output (respects NO_COLOR)
- Visual indicators (carets pointing to error positions)
- Helpful hints and suggestions
- Commands to run for more information

## Findings

### Silent Unwraps in Core Algorithms

- **Issue:** Production code uses `.unwrap()` in critical paths
- **Location:**
  - `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/radix.rs:19, 42, 48` - `dictionary.encode_digit().unwrap()`
  - `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/chunked.rs:57, 70, 78` - `dictionary.encode_digit().unwrap()`
  - `/home/kautau/work/personal/code/base-d/src/cli/commands.rs:97, 110` - `.choose(&mut rng).unwrap()`
  - `/home/kautau/work/personal/code/base-d/src/features/compression.rs:31` - Random algorithm selection
- **Impact:** Could panic at runtime if dictionary validation logic has bugs
- **Recommendation:** Replace with proper error propagation or document invariants that make unwrap safe
- **Priority:** **Medium** - Protected by validation in Dictionary construction, but brittle

### Unsafe Environment Variable Access in Tests

- **Issue:** Tests use `unsafe` blocks to manipulate `NO_COLOR` environment variable
- **Location:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/errors.rs:337-356, 362-383, 389-405`
- **Impact:** Tests are not thread-safe, could cause flaky test failures or data races
- **Recommendation:** Use scoped environment variable utilities (e.g., `temp-env` crate) or run tests serially
- **Priority:** **Low** - Only affects test reliability, not production

### Panics in Test Code (Acceptable)

- **Issue:** Many `panic!` calls in test functions and SIMD test code
- **Location:**
  - `/home/kautau/work/personal/code/base-d/src/simd/x86_64/specialized/base16.rs:680-703` (3 occurrences)
  - `/home/kautau/work/personal/code/base-d/src/simd/x86_64/specialized/base256.rs:327-419` (14 occurrences)
  - `/home/kautau/work/personal/code/base-d/src/simd/aarch64/specialized/` (similar pattern)
  - `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/` (test assertions)
- **Impact:** None - appropriate for test code
- **Recommendation:** No action needed - this is idiomatic Rust test code
- **Priority:** **None**

### ByteRange Mode Expects

- **Issue:** Uses `.expect()` for required configuration
- **Location:**
  - `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/byte_range.rs:9` - "ByteRange mode requires start_codepoint"
  - `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/byte_range.rs:41` - Same message
- **Impact:** Will panic if Dictionary is misconfigured, but this is validated during construction
- **Recommendation:** Document the invariant or refactor to use enum variants that enforce this at compile time
- **Priority:** **Low** - Protected by Dictionary validation

### Exit Code Inflexibility

- **Issue:** All errors exit with code 1 regardless of error type
- **Location:** `/home/kautau/work/personal/code/base-d/src/main.rs:6`
- **Impact:** Shell scripts cannot distinguish between different failure modes
- **Recommendation:** Map error types to meaningful exit codes (see table below)
- **Priority:** **Low** - Nice to have for better shell integration

## Missing Context

### Generic String Errors

- **Issue:** Some functions return string errors instead of typed errors
- **Location:**
  - `/home/kautau/work/personal/code/base-d/src/core/dictionary.rs` - Returns `Result<T, String>`
  - `/home/kautau/work/personal/code/base-d/src/features/compression.rs:56` - Unknown algorithm error
- **Impact:** Harder to programmatically handle specific error cases
- **Recommendation:** Consider creating an `EncodingError` enum to unify library errors
- **Priority:** **Low** - Current approach is functional, but could be more idiomatic

### Schema Parsing Unwraps

- **Issue:** Schema parsers use `.unwrap()` on JSON number conversions
- **Location:**
  - `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/parsers/json.rs:233` - `obj.into_iter().next().unwrap()`
  - `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/parsers/json.rs:629, 636, 639` - `.as_f64().unwrap()`
- **Impact:** Could panic on malformed JSON that passes serde validation
- **Recommendation:** Add defensive checks or wrap in Result with proper error
- **Priority:** **Medium** - User-provided input should never panic

## Log Quality Assessment

| Level | Used Appropriately | Issues |
|-------|-------------------|--------|
| ERROR | ✅ Uses `eprintln!` for errors | No structured logging framework |
| WARN  | ✅ Used for size limit warnings | Informal, not filterable |
| INFO  | ✅ Used for informational messages (random selection notes) | Mixed with warnings on stderr |
| DEBUG | ❌ Not implemented | No debug/trace infrastructure |
| TRACE | ❌ Not implemented | Would help debug SIMD paths |

**Assessment:**
- Basic stderr logging is functional but not production-grade
- Cannot filter log levels at runtime
- No request correlation or tracing
- Cannot debug issues without recompilation
- Good: Separates user output (stdout) from diagnostic output (stderr)

## Exit Code Audit

| Code | Current Meaning | Documented | Recommendation |
|------|----------------|------------|----------------|
| 0    | Success | Implicit | ✅ Keep |
| 1    | Any error | Implicit | Split by error type |

### Recommended Exit Code Strategy

| Code | Meaning | Use Case |
|------|---------|----------|
| 0    | Success | Normal operation |
| 1    | General error | Unknown/unclassified errors |
| 2    | Invalid arguments | CLI parsing errors, invalid dictionary name |
| 3    | Invalid input | Decode errors, malformed data |
| 4    | I/O error | File not found, permission denied |
| 65   | Input too large | File exceeds --max-size without --force |
| 70   | Internal error | Unexpected panics (catch_unwind) |

## Recommendations

### Immediate

1. **Remove unwraps in schema JSON parsing** (`/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/parsers/json.rs`)
   - Lines 233, 629, 636, 639 handle user input and should never panic
   - Return proper `SchemaError::InvalidInput` instead
   - **Risk:** User-provided malformed JSON can crash the application

2. **Audit dictionary.encode_digit() unwraps**
   - Document why these are safe (validated during Dictionary construction)
   - Consider adding debug_assert! to catch validation bugs in development
   - Alternative: Make encode_digit infallible by design

### Short-term

3. **Add structured logging framework**
   - Use `tracing` crate for structured, filterable logs
   - Add trace-level logs for SIMD path selection
   - Include debug logs for algorithm selection and chunk processing
   - Would significantly improve debuggability in production

4. **Implement meaningful exit codes**
   - Parse error types and map to conventional exit codes
   - Update documentation with exit code meanings
   - Helps shell scripts handle errors appropriately

5. **Fix unsafe test environment access**
   - Replace `std::env::set_var` in tests with thread-safe alternatives
   - Use `temp-env` crate or similar
   - Run tests with `RUST_TEST_THREADS=1` as workaround

### Long-term

6. **Add observability layer**
   - Consider `tracing` + `tracing-subscriber` for production logging
   - Add span context for request tracing (if used as library)
   - Implement error metrics (count by type)
   - Would enable production debugging without code changes

7. **Unify error types**
   - Create `base_d::Error` enum that wraps all error types
   - Implement `From` conversions from component errors
   - Provides single error type for library consumers
   - Makes error handling more ergonomic

8. **Add retry logic for I/O operations**
   - Streaming encoder/decoder could retry on transient failures
   - Exponential backoff for network-like scenarios
   - Currently fails immediately on any I/O error

## What's Good

**Excellent error messages:**
- The `DecodeError` Display implementation is exemplary (colors, carets, hints)
- `DictionaryNotFoundError` with Levenshtein suggestions is user-friendly
- Error messages tell users exactly what went wrong and how to fix it

**Strong error typing:**
- Custom error enums (`DecodeError`, `SchemaError`) capture all failure modes
- `std::error::Error` trait implemented correctly
- Good use of error context (position, input excerpt, valid characters)

**Proper error propagation:**
- Consistent use of `Result` and `?` operator throughout
- No error swallowing detected in production code paths
- Errors bubble up cleanly from lib to CLI to main

**Separation of concerns:**
- Library returns typed errors, CLI handles user presentation
- stderr for diagnostics, stdout for data - proper Unix conventions
- Error Display respects NO_COLOR environment variable

**Input validation:**
- Dictionary construction validates all constraints upfront
- File size checks with override flag (--force)
- Control character detection before terminal output
- Prevents many error conditions at the boundary

**Schema error design:**
- `SchemaError` variants include position and context
- Type mismatches show expected vs actual types
- Structured for both humans and programmatic handling

Knock knock, Neo.
