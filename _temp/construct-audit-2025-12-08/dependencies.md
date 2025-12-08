# Dependencies

## Summary

Base-d has a **healthy dependency profile** overall. The project uses 33 direct dependencies (29 runtime, 4 dev) expanding to 198 transitive packages. All dependencies are from trusted sources (crates.io), use permissive licenses (MIT/Apache-2.0 dual licensing dominates), and follow semantic versioning with caret ranges for safe minor updates.

**Key Health Indicators:**
- **Security:** No automated audit tool available (cargo-audit installation issue), manual review shows no obvious red flags
- **Currency:** Majority of deps are current; 23 minor/patch updates available
- **Maintenance:** Core deps (RustCrypto hashes, compression libs) are actively maintained
- **License Compliance:** All compatible with MIT/Apache-2.0, except cargo-husky (dev-only, no license specified)
- **Versioning Strategy:** Caret ranges (^) - good balance of stability and security updates

**Concerns:**
1. **Medium Risk:** `markdown` dependency using alpha version (1.0.0-alpha.21)
2. **Low Risk:** `k12` hash is experimental/unstable API per upstream docs
3. **Low Risk:** `criterion` has major update available (0.7 → 0.8.1) with breaking changes

---

## Security Audit Results

**Tool:** cargo-audit (attempted)
**Status:** ⚠️ **Unable to run** - binary exists but fails to execute (corrupted installation)

**Alternative Check:** Manual review via cargo outdated, metadata, and lock file analysis
- **Critical:** 0 (none identified)
- **High:** 0 (none identified)
- **Medium:** 0 (none identified)
- **Low:** 0 (none identified)

**Recommendation:** Reinstall cargo-audit or use alternative like `cargo deny` to perform automated CVE scans. Without automated scanning, cannot guarantee absence of known vulnerabilities.

---

## Vulnerability Details

| Package | Version | CVE | Severity | Fixed In | Action |
|---------|---------|-----|----------|----------|--------|
| *(none found via manual review)* | - | - | - | - | Install working audit tool |

**Note:** This is NOT a comprehensive security audit. Proper tooling (cargo-audit, trivy, or cargo-deny) is required for CVE scanning against RustSec advisory database.

---

## Outdated Packages

From `cargo outdated` output:

| Package | Current | Latest | Breaking? | Risk | Notes |
|---------|---------|--------|-----------|------|-------|
| **criterion** | 0.7.0 | 0.8.1 | ⚠️ Yes (0.6→0.8) | Low | Dev-only; major API changes expected |
| criterion-plot | 0.6.0 | 0.8.1 | ⚠️ Yes | Low | Transitive via criterion |
| **k12** | 0.2.1 | 0.3.0 | ⚠️ Likely | Low | Experimental API, expect churn |
| **crc** | 3.3.0 | 3.4.0 | ❌ No | Low | Minor update, safe |
| wasm-bindgen (all) | 0.2.105 | 0.2.106 | ❌ No | Low | Patch update |
| web-sys | 0.3.82 | 0.3.83 | ❌ No | Low | Patch update |
| js-sys | 0.3.82 | 0.3.83 | ❌ No | Low | Patch update |
| winnow | 0.7.13 | 0.7.14 | ❌ No | Low | Parser lib (via toml), patch |
| zerocopy | 0.8.28 | 0.8.31 | ❌ No | Low | Patch updates |
| cc | 1.2.47 | 1.2.49 | ❌ No | Low | Build-time only |
| simd-adler32 | 0.3.7 | 0.3.8 | ❌ No | Low | Transitive via miniz_oxide |

**Safe to Update (non-breaking):**
- crc (3.3.0 → 3.4.0)
- All wasm-bindgen ecosystem (0.2.105 → 0.2.106)
- winnow (0.7.13 → 0.7.14)
- zerocopy (0.8.28 → 0.8.31)
- cc, simd-adler32 (build/transitive)

**Evaluate Before Updating (breaking):**
- criterion (0.7 → 0.8.1) - Dev only, check benchmark compatibility
- k12 (0.2.1 → 0.3.0) - Experimental hash, review API changes

---

## Dependency Tree Health

**Direct dependencies:** 33 (29 runtime, 4 dev)
**Transitive dependencies:** 198 (from crates.io registry)
**Total packages:** 199 (including base-d itself)
**Duplicate packages:** 0 (cargo tree --duplicates found none)
**Dependency relationships:** ~184 edges (depth 3)
**Lock file size:** 45KB
**Build artifacts:** 561 MB (target/ directory)

**Breakdown:**
```
Runtime (29):
  Crypto: ascon-hash, blake2, blake3, sha2, sha3, md-5, k12, twox-hash, crc, hex
  Compression: flate2, brotli, zstd, lz4, snap, xz2
  CLI: clap, crossterm, terminal_size, dirs, shellexpand
  Serialization: serde, serde_json, toml
  Math: num-bigint, num-integer, num-traits
  Misc: rand, markdown

Dev-only (4):
  criterion, assert_cmd, predicates, cargo-husky
```

**Transitive Depth:** Shallow to moderate (most deps <5 levels deep). No pathological dependency trees observed.

---

## Maintenance Concerns

| Package | Last Release | Maintainers | Concern | Severity |
|---------|--------------|-------------|---------|----------|
| **markdown** | 1.0.0-alpha.21 | wooorm (markdown-rs) | Alpha/pre-release in production | Medium |
| **k12** | 0.2.1 | RustCrypto | Experimental, unstable API warning | Low |
| cargo-husky | 1.5.0 | rhysd | No license specified in metadata | Low |

**Detailed Analysis:**

### 🟡 markdown (1.0.0-alpha.21)
- **Status:** Pre-1.0 alpha release in runtime dependencies
- **Repository:** https://github.com/wooorm/markdown-rs (active, 20+ contributors)
- **Risk:** API stability not guaranteed; potential breaking changes before 1.0
- **Recommendation:** Monitor for 1.0 stable release; consider if markdown parsing is critical path

### 🟡 k12 (KangarooTwelve hash)
- **Status:** Experimental per RustCrypto docs
- **Repository:** Part of RustCrypto/hashes (well-maintained org)
- **Risk:** API marked unstable, may change between minor versions
- **Recommendation:** Acceptable for non-critical hashing; avoid if security-critical without expert review

### 🟢 cargo-husky (1.5.0)
- **Status:** Dev-dependency only, no license in metadata (likely MIT based on repo)
- **Repository:** https://github.com/rhysd/cargo-husky (maintained by rhysd)
- **Risk:** Minimal (dev-time git hooks only)
- **Recommendation:** Verify license from repo if distributing; otherwise no action needed

**No Abandoned Dependencies:** All major deps show recent activity (<6 months). RustCrypto, compression libraries (brotli, zstd, lz4), and CLI tools (clap, crossterm) are actively maintained.

---

## License Audit

| License | Count | Compatible | Packages |
|---------|-------|------------|----------|
| MIT OR Apache-2.0 | 178 | ✅ Yes | Majority (standard Rust dual-license) |
| BSD-3-Clause | 3 | ✅ Yes | brotli, snap, subtle |
| BSD-2-Clause | 2 | ✅ Yes | arrayref, zerocopy |
| Unlicense OR MIT | 4 | ✅ Yes | aho-corasick, memchr, same-file, walkdir |
| CC0-1.0 OR Apache-2.0 | 1 | ✅ Yes | blake3 |
| MPL-2.0 | 1 | ✅ Yes | option-ext |
| 0BSD OR MIT OR Apache-2.0 | 1 | ✅ Yes | adler2 |
| Unicode-3.0 | 1 | ✅ Yes | unicode-ident (combined with MIT/Apache) |
| **NONE** (unspecified) | 1 | ⚠️ Check | **cargo-husky** (dev-only) |

**License Compatibility Assessment:**
- ✅ **All runtime dependencies** use permissive licenses compatible with MIT/Apache-2.0
- ✅ **No copyleft (GPL/LGPL)** licenses that would require disclosure or source distribution
- ⚠️ **cargo-husky** lacks license in metadata but is dev-dependency only (not distributed)
- ✅ **BSD, Unlicense, CC0, MPL-2.0** all compatible with project's MIT OR Apache-2.0 license

**Compliance Status:** **PASS** - No licensing blockers for distribution.

---

## Recommendations

### Update Immediately
*(None - no critical security vulnerabilities identified)*

**Action:** Install working cargo-audit tool to enable automated vulnerability scanning:
```bash
cargo install --force cargo-audit  # Reinstall
# OR
cargo install cargo-deny          # Alternative with more features
```

### Update Soon (Safe, Non-Breaking)
1. **wasm-bindgen ecosystem** (0.2.105 → 0.2.106) - Patch update
2. **web-sys, js-sys** (0.3.82 → 0.3.83) - Patch update
3. **crc** (3.3.0 → 3.4.0) - Minor version, safe
4. **winnow** (0.7.13 → 0.7.14) - Parser lib via toml
5. **zerocopy** (0.8.28 → 0.8.31) - Patch updates

**Command:** `cargo update` (updates within caret ranges automatically)

### Investigate & Evaluate
1. **markdown (1.0.0-alpha.21)** - Pre-release dependency
   - Check for 1.0 stable release availability
   - Review if markdown parsing is critical to functionality
   - Consider risk of API changes in alpha software

2. **k12 (0.2.1 → 0.3.0)** - Experimental hash function
   - Review upstream RustCrypto release notes for API changes
   - Evaluate if k12 is essential vs. stable alternatives (SHA3, BLAKE3)
   - Consider replacing with stable hash if not specifically required

3. **criterion (0.7.0 → 0.8.1)** - Dev-dependency only
   - Check benchmark compatibility before upgrading
   - Breaking changes expected (0.6 → 0.8 jump)
   - Non-urgent: only affects benchmarking, not runtime

### Remove
*(None identified - no unused dependencies detected)*

**To verify:** Install `cargo-udeps` for automated unused dependency detection:
```bash
cargo install cargo-udeps
cargo +nightly udeps
```

### Replace
*(No immediate candidates)*

**Potential Future Consideration:**
- **markdown** - If stability becomes issue, evaluate alternatives like `pulldown-cmark` (mature, stable 0.12.x)
- **k12** - If experimental status is concern, consolidate on stable hashes (SHA3, BLAKE3 already present)

---

## What's Good

✅ **Clean dependency hygiene:**
- Zero duplicate dependencies across entire tree
- Shallow transitive dependency depth (no bloat)
- Sensible versioning strategy (caret ranges for flexibility + safety)

✅ **Trusted sources:**
- All deps from crates.io registry
- Heavy use of RustCrypto ecosystem (industry standard)
- Well-known, maintained compression libraries

✅ **License compliance:**
- 100% permissive licensing (MIT/Apache-2.0 dominated)
- No copyleft entanglements
- Clear compatibility with project license

✅ **Focused dependency set:**
- Each dependency serves clear purpose (crypto, compression, CLI, serialization)
- No redundant capabilities (e.g., single HTTP client, single CLI framework)
- Dev dependencies properly separated

✅ **Lock file committed:**
- Cargo.lock present and tracked
- Enables reproducible builds
- Lock file size reasonable (45KB)

✅ **Modern Rust practices:**
- Using 2024 edition (latest stable)
- No deprecated crates identified
- Following semantic versioning conventions

---

**Knock knock, Neo.**
