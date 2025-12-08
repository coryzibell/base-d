# Technical Debt

## Summary

base-d is a well-maintained project with remarkably low technical debt for a 33k+ LOC codebase. The code is modern (Rust 2024 edition), passes clippy with no warnings, and has good test coverage. However, there are some areas that need attention:

**Overall Debt Assessment:** **Low to Medium**

The primary debt is **inadvertent/prudent** - things discovered as the project evolved that would be better done differently now. There's very little reckless debt. Most TODOs are legitimate feature gaps (UTF-8 encoding, remainder handling) rather than hacks or workarounds.

**Velocity Impact:** The current debt is unlikely to slow most development. The main friction points are:
1. Large files (2000+ LOC) making navigation harder
2. Scattered TODOs that could be prioritized
3. Some deprecated API methods still in use internally

---

## Debt Inventory

| Type | Count | Severity | Trend |
|------|-------|----------|-------|
| TODO/FIXME comments | 74 | Low-Medium | Stable |
| Deprecated methods | 5 (in core/dictionary.rs) | Low | Stable |
| `#[allow(deprecated)]` usage | 17 files | Low | Stable |
| `unwrap()` calls | 898 | Medium | Unknown |
| `expect()` calls | 49 | Low | Unknown |
| `unsafe` blocks/calls | 618 | Low* | Stable |
| Large files (>1000 LOC) | 5 files | Medium | Growing |
| Very large files (>2000 LOC) | 3 files | High | Growing |
| `clone()` usage | ~30 instances | Low | Stable |

*Unsafe is expected and appropriate for SIMD code - not debt unless lacking safety docs

---

## High-Interest Debt

**Things that slow down every change - pay these first:**

### 1. Very Large Files (2000+ LOC)
**Severity:** High
**Files:**
- `src/simd/lut/base64.rs` (2510 LOC)
- `src/encoders/algorithms/schema/fiche.rs` (2440 LOC)
- `src/simd/generic/mod.rs` (2281 LOC)

**Impact:** These files are difficult to navigate, review, and reason about. Changes require extensive context loading.

**Why it matters:** When files exceed 2000 lines, cognitive load increases significantly. Code reviews take longer, bugs are harder to spot, and new contributors struggle to understand the scope.

**Recommendation:**
- Extract test modules to separate files (`mod tests` in `tests/simd_lut_base64.rs`)
- Split codec implementations into sub-modules (e.g., `base64/encode.rs`, `base64/decode.rs`, `base64/tests.rs`)
- For `fiche.rs`: separate parsing/serialization/compression into distinct modules

---

### 2. Scattered TODO Comments (74 total)
**Severity:** Medium
**Pattern:** Most TODOs are in `src/simd/generic/mod.rs` (58 of 74)

**Location breakdown:**
- SIMD remainder handling: ~50 TODOs ("Handle remainder with scalar")
- UTF-8 encoding for chars > 0x7F: 3 TODOs
- Environment access audits: 6 TODOs (in `errors.rs`)
- LUT decode failure for large inputs: 1 TODO

**Impact:** TODOs without tracking or prioritization become invisible. The "remainder handling" TODOs represent incomplete SIMD implementation that may cause silent performance degradation.

**Why it matters:** When encoding data that doesn't align to SIMD block boundaries (which is common), the code currently ignores remainder bytes. This could lead to:
- Incorrect output for certain input sizes
- Silent data truncation
- Confusing bugs

**Recommendation:**
1. **Immediate:** Verify that remainder bytes are handled at a higher level (they probably are, but needs audit)
2. **Short-term:** Add tests that verify behavior for non-aligned input sizes (e.g., 13 bytes, 17 bytes)
3. **Medium-term:** Implement scalar fallback for remainders and remove TODOs
4. **Process:** Convert TODOs to tracked issues with priority labels

---

### 3. Deprecation Debt (not cleaned up)
**Severity:** Low
**Location:** `src/core/dictionary.rs`

**What:** Five deprecated constructor methods (`new()`, `new_with_mode()`, `new_with_mode_and_range()`, `from()`) are marked `#[deprecated]` but still used internally in 17 files via `#[allow(deprecated)]`.

**Why it matters:** Deprecation warnings exist to drive migration. When internal code uses `#[allow(deprecated)]`, it signals:
- The new API isn't actually better for all cases (design smell)
- Migration effort was abandoned mid-way
- Future removal is blocked

**Recommendation:**
- Audit all `#[allow(deprecated)]` usage
- Migrate internal code to `Dictionary::builder()` pattern
- If builder pattern is insufficient, reconsider the deprecation
- Document migration path in CHANGELOG

---

## Quick Wins

**Easy refactors with high impact - low cost, high value:**

### 1. Extract Test Modules (Effort: 1 hour each)
**Files:** `src/simd/lut/base64.rs`, `src/simd/generic/mod.rs`

Move `#[cfg(test)] mod tests { ... }` blocks to separate files. Reduces cognitive load when reading implementation code.

**Before:**
```
src/simd/lut/base64.rs (2510 LOC)
```

**After:**
```
src/simd/lut/base64.rs (1500 LOC)
tests/simd_lut_base64.rs (1010 LOC)
```

---

### 2. Audit and Document `unwrap()` Usage (Effort: 4 hours)
**Count:** 898 calls

Run `cargo clippy -- -W clippy::unwrap_used` to identify all `unwrap()` calls, then:
- Tests/benchmarks: acceptable (mark with `#[allow]` if needed)
- Library code with invariants: add `expect("reason")`
- Library code on user input: convert to `Result`

Most are probably safe, but 898 is high enough to warrant a review pass.

---

### 3. Categorize TODOs by Priority (Effort: 2 hours)
Convert inline TODOs to GitHub issues with labels:
- `P0-critical`: Correctness bugs (e.g., remainder handling if broken)
- `P1-high`: Performance gaps (e.g., UTF-8 encoding)
- `P2-medium`: Nice-to-have improvements
- `P3-low`: Future optimizations

Remove TODOs from code, reference issue numbers in comments instead:
```rust
// See #123 for remainder handling implementation
```

---

### 4. Add Input Size Invariant Tests (Effort: 3 hours)
**Target:** `src/simd/generic/mod.rs`

Add property-based tests or edge case tests for:
- Input sizes: 0, 1, 15, 16, 17, 31, 32, 33 bytes
- Verify no silent truncation
- Verify SIMD matches scalar output

---

## Findings

### File Size Bloat
- **What:** Three files exceed 2000 LOC; five exceed 1000 LOC
- **Type:** Inadvertent/Prudent (grew organically as features were added)
- **Why it matters:** Large files increase cognitive load, slow reviews, and make refactoring risky
- **Location:**
  - `src/simd/lut/base64.rs` (2510 LOC)
  - `src/encoders/algorithms/schema/fiche.rs` (2440 LOC)
  - `src/simd/generic/mod.rs` (2281 LOC)
  - `src/simd/lut/gapped.rs` (1685 LOC)
  - `src/simd/x86_64/specialized/base32.rs` (1004 LOC)
- **Effort to fix:** Medium (4-8 hours per file for extraction)
- **Recommendation:** Extract tests, split codec phases (encode/decode/translate), create sub-modules

---

### Incomplete SIMD Implementation (Remainder Handling)
- **What:** 50+ TODOs in `src/simd/generic/mod.rs` about handling remainder bytes
- **Type:** Inadvertent/Prudent (discovered after SIMD implementation that edge cases needed handling)
- **Why it matters:** If remainders aren't handled elsewhere, this could cause:
  - Data truncation for non-aligned inputs
  - Incorrect encoding/decoding
  - Silent failures
- **Location:** `src/simd/generic/mod.rs` lines 182, 222, 241, 290, 306, 338, 356, 396, 414, 472, 490, 523, 541, 569, 587, 620, 839, 885, 931, 972, 1030, 1073, 1116, 1154, 1378, 1448, 1510, 1566, 1651, 1726, 1784, 1836
- **Effort to fix:** Large (2-3 days to implement scalar fallback for all variants)
- **Recommendation:**
  1. Audit that remainders are handled at higher level (likely in chunked/radix algorithms)
  2. Add explicit tests for non-aligned input sizes
  3. Implement scalar fallback for remainders or document why it's safe to skip
  4. Convert TODOs to tracked issues

---

### UTF-8 Encoding Gap
- **What:** SIMD codecs only support ASCII (< 0x80), multi-byte UTF-8 not implemented
- **Type:** Inadvertent/Prudent (MVP shipped with ASCII, UTF-8 needed for full Unicode dictionaries)
- **Why it matters:** Limits dictionaries to ASCII characters, preventing use of full Unicode range for base256+ encodings
- **Location:**
  - `src/simd/generic/mod.rs:2088` ("Implement proper UTF-8 encoding for chars > 0x7F")
  - `src/simd/generic/mod.rs:2105` ("Add UTF-8 encoding support for higher Unicode ranges")
- **Effort to fix:** Medium (2-3 days for implementation + testing)
- **Recommendation:**
  - Document limitation clearly in README/docs
  - Add as tracked feature request
  - Consider using external crate like `simdutf8` or `encoding_rs`

---

### Deprecated API Not Removed
- **What:** 5 deprecated constructor methods in `Dictionary` still used internally
- **Type:** Deliberate/Prudent (new builder pattern introduced but old API kept for compatibility)
- **Why it matters:**
  - Internal code using deprecated APIs via `#[allow(deprecated)]` blocks future removal
  - Signals incomplete migration or insufficient new API
  - Users see deprecation warnings but internal code ignores them
- **Location:** `src/core/dictionary.rs:51-104`
- **Effort to fix:** Small (4-6 hours to migrate all internal usage)
- **Recommendation:**
  - Complete migration to builder pattern throughout codebase
  - Remove `#[allow(deprecated)]` suppressions
  - Add timeline for removal in deprecation notice (e.g., "will be removed in v4.0")
  - If builder pattern is insufficient, reconsider deprecation

---

### Environment Variable Access (Audit Needed)
- **What:** 6 TODOs about auditing environment variable access in error handling
- **Type:** Deliberate/Prudent (shipped knowing that env access needs thread-safety audit)
- **Why it matters:** `std::env::var()` can panic if environment is modified during access (rare but possible in multi-threaded contexts)
- **Location:** `src/encoders/algorithms/errors.rs:338, 352, 361, 379, 388, 401`
- **Effort to fix:** Small (2 hours for audit + fix)
- **Recommendation:**
  - Verify these code paths only run in single-threaded contexts (likely CLI)
  - Use `std::env::var_os()` instead (doesn't panic on invalid UTF-8)
  - Cache environment variable values at startup instead of checking repeatedly

---

### Clone Usage (Potential Performance Impact)
- **What:** ~30 `clone()` calls, mostly on `String` and `Vec`
- **Type:** Inadvertent/Prudent (not all clones are avoidable, but some could be references)
- **Why it matters:** Unnecessary clones in hot paths impact performance
- **Location:** Mostly in:
  - `src/cli/handlers/` (likely fine, CLI code)
  - `src/encoders/algorithms/schema/` (could impact schema encoding perf)
  - `src/features/detection.rs` (called during auto-detection, could be hot)
- **Effort to fix:** Medium (4-6 hours to audit and optimize)
- **Recommendation:**
  - Run `cargo clippy -- -W clippy::clone_on_ref_ptr` to identify unnecessary clones
  - Profile schema encoding/detection to see if clones appear in hot paths
  - Consider `Cow<str>` or `&str` where ownership isn't needed

---

### Large Number of `unwrap()` Calls (898)
- **What:** 898 calls to `unwrap()`, 49 calls to `expect()`
- **Type:** Mixed (tests are fine, library code may have invariants or lack error handling)
- **Why it matters:**
  - Library code `unwrap()` on user input = potential panics
  - Missing error context makes debugging harder
- **Location:** Throughout codebase (need targeted audit)
- **Effort to fix:** Large (8-12 hours for full audit and fixes)
- **Recommendation:**
  1. Enable `clippy::unwrap_used` lint to prevent new unwraps
  2. Audit library code (non-test) for unwraps on:
     - User input (convert to `Result`)
     - External data (convert to `Result`)
     - Invariants (document with `expect("why this is safe")`)
  3. Tests/benches can keep `unwrap()`

---

### High Unsafe Usage (618 instances)
- **What:** 618 `unsafe` blocks/calls, primarily in SIMD code
- **Type:** Appropriate for domain (SIMD requires unsafe), but needs safety documentation
- **Why it matters:**
  - SIMD intrinsics require `unsafe` - this is expected
  - Safety comments should document why each `unsafe` is sound
- **Location:** Concentrated in `src/simd/`
- **Effort to fix:** Large (12-16 hours to document all safety invariants)
- **Recommendation:**
  - Audit that each `unsafe` block has a `// SAFETY:` comment
  - Run `cargo miri test` to catch undefined behavior
  - Consider using higher-level SIMD abstractions (e.g., `wide`, `safe_arch`) where possible
  - Enable `clippy::undocumented_unsafe_blocks` lint

---

### Reusable Workflow Dependency
- **What:** CI uses external workflow `coryzibell/nebuchadnezzar/.github/workflows/wake-up.yml@main`
- **Type:** Deliberate/Prudent (centralizing CI logic is good, but coupling to external repo has risks)
- **Why it matters:**
  - Breaking changes in `nebuchadnezzar` could break CI
  - `@main` branch reference means no version pinning
  - External repo could be deleted/renamed/made private
- **Location:** `.github/workflows/wake-up.yml:16`
- **Effort to fix:** Small (1 hour to vendor or pin version)
- **Recommendation:**
  - Pin to specific commit or tag: `nebuchadnezzar/.github/workflows/wake-up.yml@v1.2.3`
  - Or vendor the workflow into this repo
  - Document the coupling in CONTRIBUTING.md

---

## Refactoring Roadmap

### Phase 1: Quick Wins (1-2 days each)

1. **Extract test modules from large files** (4 hours)
   - Move test code out of `base64.rs`, `generic/mod.rs`, `fiche.rs`
   - Reduces file sizes by 30-40%

2. **Categorize and track TODOs** (2 hours)
   - Create GitHub issues for all 74 TODOs
   - Remove inline TODOs, reference issues instead
   - Prioritize remainder handling and UTF-8 encoding

3. **Audit environment variable access** (2 hours)
   - Fix the 6 TODOs in `errors.rs`
   - Cache env vars or verify thread safety

4. **Add input size edge case tests** (3 hours)
   - Test SIMD with sizes: 0, 1, 15, 16, 17, 31, 32, 33, 63, 64, 65 bytes
   - Verify no silent truncation

### Phase 2: Medium Effort (1 week each)

1. **Implement SIMD remainder handling** (3 days)
   - Add scalar fallback for non-aligned input sizes
   - Remove the 50+ remainder TODOs
   - Comprehensive testing

2. **Complete deprecated API migration** (1 day)
   - Migrate all internal code to builder pattern
   - Remove `#[allow(deprecated)]` suppressions
   - Schedule removal in next major version

3. **Audit and optimize clone usage** (1 day)
   - Profile schema encoding and detection paths
   - Replace unnecessary clones with references
   - Use `Cow` where appropriate

4. **Document unsafe blocks** (2 days)
   - Add `// SAFETY:` comments to all unsafe usage
   - Run `cargo miri test`
   - Enable `clippy::undocumented_unsafe_blocks`

### Phase 3: Large Refactors (multi-week)

1. **Split large files into sub-modules** (1-2 weeks)
   - `simd/lut/base64.rs` → `simd/lut/base64/{encode,decode,tests}.rs`
   - `encoders/algorithms/schema/fiche.rs` → `schema/fiche/{parse,serialize,compress}.rs`
   - `simd/generic/mod.rs` → `simd/generic/{encode,decode,tests}.rs`

2. **Implement UTF-8 SIMD support** (1 week)
   - Research: evaluate `simdutf8` vs custom implementation
   - Implement multi-byte UTF-8 encoding in SIMD codecs
   - Enable full Unicode dictionary support
   - Comprehensive testing

3. **Comprehensive `unwrap()` audit** (1 week)
   - Enable `clippy::unwrap_used` lint
   - Audit all 898 unwrap calls
   - Convert library unwraps to proper error handling
   - Document test/bench exceptions

---

## Debt Backlog

| Item | Effort | Value | Priority | Issue |
|------|--------|-------|----------|-------|
| Extract test modules | Small | High | P1 | - |
| Track TODOs as issues | Small | High | P1 | - |
| Test SIMD input size edge cases | Small | High | P0 | - |
| Audit env var access | Small | Medium | P2 | - |
| Implement remainder handling | Large | High | P1 | - |
| Complete deprecated migration | Small | Medium | P2 | - |
| Audit clone usage | Medium | Medium | P2 | - |
| Document unsafe blocks | Large | Medium | P2 | - |
| Split large files | Large | Medium | P2 | - |
| UTF-8 SIMD support | Large | Low | P3 | - |
| Audit unwrap usage | Large | Medium | P2 | - |
| Pin CI workflow version | Small | Low | P3 | - |

---

## What's Good

**Well-maintained areas, good practices to preserve:**

### Modern Rust (2024 Edition)
The project uses Rust 2024 edition, showing commitment to staying current with language improvements.

### Clean Clippy
Zero clippy warnings out of the box. This is exceptional for a 33k LOC codebase with extensive unsafe SIMD code.

### Good Test Coverage
14 passing doc tests, comprehensive integration tests in `tests/`, and benchmark suite. Tests are ignored appropriately (not removed), showing intentional maintenance.

### SIMD Architecture
The SIMD cascade (specialized → generic → LUT → scalar fallback) is well-designed. Clear separation of concerns between x86_64/aarch64 implementations.

### Feature Flags
Minimal, focused feature flags (`simd` only). Avoids feature flag explosion.

### Comprehensive Documentation
Extensive docs in `docs/` covering all major features. README is detailed and up-to-date. API docs exist for public interfaces.

### External Workflow Reuse
Using `nebuchadnezzar` for CI shows good DRY principles (even if it could be pinned better).

### Error Types
Dedicated error types (`DecodeError`, `DictionaryNotFoundError`, `SchemaError`) with context. Not just using `Box<dyn Error>` everywhere.

### Builder Pattern
`Dictionary::builder()` is a clean, ergonomic API (even if the old API isn't fully removed yet).

### Minimal Dependencies
33 dependencies is reasonable for the feature set (7 compression algorithms, 15 hash algorithms, SIMD, CLI). No obvious bloat.

### Consistent Naming
File and module naming follows Rust conventions. No Java-style `IEncoderFactory` or Go-style stuttering.

### No Dead Code
No obvious dead code or commented-out blocks (checked with grep). Code is actively maintained.

### Schema Subsystem Design
The IR layer in `encoders/algorithms/schema/` is well-architected: clean separation between parsers, IR, binary layer, and serializers. This is production-quality design.

---

## Notes

### Debt is Mostly Inevitable, Not Reckless
Almost all debt identified is "inadvertent/prudent" - things learned after shipping that would be done differently now. Examples:
- SIMD remainder handling wasn't prioritized in MVP
- UTF-8 support deferred for ASCII-first release
- Files grew large as features accumulated

There's very little "reckless" debt (hacks, quick fixes, "we'll fix it later" shortcuts).

### Some "Debt" Isn't Debt
- **Unsafe in SIMD:** Required for intrinsics, not avoidable debt
- **Large files:** Sometimes complex algorithms just take space (though extraction still helps)
- **TODOs:** Many are legitimate feature requests, not deferred fixes

### Active Development vs Stagnation
The presence of TODOs and incomplete features (UTF-8, remainder handling) suggests active development, not abandonment. This is healthy - ship fast, iterate.

### Comparison to Similar Projects
For a project doing low-level SIMD optimization with 33k LOC:
- **unwrap count (898):** Higher than ideal, but not alarming if most are in tests/invariants
- **unsafe count (618):** Expected for SIMD-heavy codebase
- **file sizes:** Base64.rs at 2510 LOC is large but not unprecedented for codec implementations

### Risk Assessment
**Low risk areas:**
- Core encoding/decoding algorithms (well-tested)
- SIMD implementations (performance-critical, likely audited)
- CLI interface (stable, clear ownership)

**Medium risk areas:**
- Schema encoding (complex, 2440 LOC in one file, harder to review)
- SIMD remainder handling (unclear if tested thoroughly)
- Deprecated API migration (incomplete, blocks cleanup)

**High risk areas:**
- None identified. No systemic issues that threaten stability.

---

**Files referenced:**
- `/home/kautau/work/personal/code/base-d/src/simd/lut/base64.rs`
- `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/fiche.rs`
- `/home/kautau/work/personal/code/base-d/src/simd/generic/mod.rs`
- `/home/kautau/work/personal/code/base-d/src/core/dictionary.rs`
- `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/errors.rs`
- `/home/kautau/work/personal/code/base-d/Cargo.toml`
- `/home/kautau/work/personal/code/base-d/.github/workflows/wake-up.yml`
