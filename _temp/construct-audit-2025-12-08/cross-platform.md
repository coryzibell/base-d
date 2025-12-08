# Cross-Platform

## Summary

base-d demonstrates **excellent cross-platform design**. The codebase uses Rust's standard abstractions and mature ecosystem crates to handle platform differences. SIMD code is properly gated by architecture, CI tests on all major platforms, and path/config handling uses platform-aware libraries. A few minor issues exist around line endings and Windows binary packaging, but core functionality is solid across Linux, macOS, and Windows.

## Platform Support

| Platform | Status | Tested | Known Issues |
|----------|--------|--------|--------------|
| Linux (x86_64) | ✅ Full | Yes (CI) | None |
| macOS (aarch64) | ✅ Full | Yes (CI) | None |
| Windows (x86_64) | ✅ Full | Yes (CI) | Missing .gitattributes (line endings) |
| Linux (aarch64) | ⚠️ Likely works | No | SIMD implemented but untested |
| Windows (aarch64) | ⚠️ Partial | No | SIMD implemented, binstall config exists |
| Other (RISC-V, etc.) | ⚠️ Fallback | No | No SIMD, scalar fallback should work |

## Portability Assessment

| Area | Portable | Issues |
|------|----------|--------|
| Paths | ✅ Yes | Uses `std::path`, `dirs` crate |
| Shell commands | ✅ Yes | CI uses native tooling per platform |
| File system | ✅ Yes | Standard Rust I/O, no permission manipulation |
| Environment | ✅ Yes | `dirs::config_dir()` handles platform conventions |
| Networking | N/A | No networking code |
| Line endings | ⚠️ Partial | Hardcoded `\r\n` in terminal output, no `.gitattributes` |
| SIMD | ✅ Yes | Proper `cfg(target_arch)` gating, runtime detection |
| Executables | ✅ Yes | Cargo handles platform binaries |

## Findings

### Paths - Excellent

- **Issue:** None
- **Platforms affected:** All
- **Location:**
  - `src/core/config.rs:234-236` - Uses `dirs::config_dir()` (cross-platform)
  - `src/core/config.rs:252` - Uses `std::path::Path::new()` for local paths
- **Evidence:**
  ```rust
  if let Some(config_dir) = dirs::config_dir() {
      let user_config_path = config_dir.join("base-d").join("dictionaries.toml");
  ```
  - Uses `PathBuf::join()` instead of string concatenation
  - No hardcoded `/` or `\` separators
  - `shellexpand` crate (line 11) handles `~` expansion portably
- **Recommendation:** No changes needed
- **Priority:** N/A (working correctly)

### SIMD Architecture Handling - Excellent

- **Issue:** None, exemplary implementation
- **Platforms affected:** x86_64, aarch64
- **Location:**
  - `src/simd/mod.rs:7-77` - Architecture-specific module imports
  - `src/simd/x86_64/` and `src/simd/aarch64/` - Separate implementations
  - `src/bench.rs:62,75` - Architecture detection for benchmarking
- **Evidence:**
  ```rust
  #[cfg(target_arch = "x86_64")]
  pub use x86_64::{encode_base64_simd, ...};

  #[cfg(target_arch = "aarch64")]
  pub use aarch64::{encode_base64_simd, ...};

  #[cfg(target_arch = "x86_64")]
  pub fn has_avx2() -> bool {
      crate::simd::x86_64::has_avx2()
  }

  #[cfg(target_arch = "aarch64")]
  pub fn has_neon() -> bool {
      true  // NEON is always available on aarch64
  }
  ```
  - Runtime CPU feature detection via `is_x86_feature_detected!`
  - Graceful fallback to scalar code when SIMD unavailable
  - No platform-specific code leaks into shared modules
- **Recommendation:** No changes needed
- **Priority:** N/A (best practice)

### Configuration Directories - Excellent

- **Issue:** None
- **Platforms affected:** All
- **Location:** `src/core/config.rs:234`, `src/cli/config.rs:15`
- **Evidence:**
  ```rust
  if let Some(config_dir) = dirs::config_dir() {
      let user_config_path = config_dir.join("base-d").join("dictionaries.toml");
  ```
  - Uses `dirs::config_dir()` which returns:
    - Linux: `~/.config/`
    - macOS: `~/Library/Application Support/`
    - Windows: `%APPDATA%`
  - Follows platform conventions automatically
- **Recommendation:** No changes needed
- **Priority:** N/A (correct implementation)

### Line Endings - Minor Issue

- **Issue:** Hardcoded `\r\n` for terminal output, missing `.gitattributes`
- **Platforms affected:** Windows (inconsistent behavior)
- **Location:** `src/cli/commands.rs:215,217,245,247,274,276,303,340,342,357,359,370,372`
- **Evidence:**
  ```rust
  eprint!("\x1b[32mDictionary: {}\x1b[0m\r\n", current_dictionary_name);
  eprint!("Dictionary: {}\r\n", current_dictionary_name);
  print!("{}\r\n", display);
  ```
  - Hardcoded CRLF (`\r\n`) in terminal output strings
  - These are for **terminal display** (Matrix mode, interactive output)
  - Intentional for proper terminal cursor control across platforms
  - However, no `.gitattributes` file to enforce line endings in source
- **Recommendation:**
  1. The `\r\n` usage is **correct** - it's for terminal control, not file I/O
  2. Add `.gitattributes` to normalize source file line endings:
     ```
     * text=auto
     *.rs text eol=lf
     *.toml text eol=lf
     *.md text eol=lf
     *.yml text eol=lf
     *.sh text eol=lf
     *.bat text eol=crlf
     *.ps1 text eol=crlf
     ```
- **Priority:** Low (cosmetic, prevents developer confusion)

### CI/CD Platform Matrix - Excellent

- **Issue:** None
- **Platforms affected:** Linux, macOS, Windows
- **Location:** `.github/workflows/wake-up.yml` (calls `nebuchadnezzar/.github/workflows/the-matrix-has-you.yml`)
- **Evidence:**
  ```yaml
  strategy:
    fail-fast: false
    matrix:
      include:
        - os: ubuntu-latest
          target: x86_64-unknown-linux-gnu
        - os: macos-latest
          target: aarch64-apple-darwin
        - os: windows-latest
          target: x86_64-pc-windows-msvc
  ```
  - Tests on all three major platforms
  - Uses `sed -i` in CI (works on Linux, may differ on macOS but CI uses Ubuntu)
  - Proper `fail-fast: false` to see all platform failures
- **Recommendation:** Consider adding macOS x86_64 target for completeness
- **Priority:** Low (current coverage is good)

### Binary Packaging - Good

- **Issue:** None, well-configured
- **Platforms affected:** Windows
- **Location:** `Cargo.toml:68-76`
- **Evidence:**
  ```toml
  [package.metadata.binstall]
  pkg-url = "{ repo }/releases/download/v{ version }/{ name }-{ target }{ archive-suffix }"
  pkg-fmt = "tgz"

  [package.metadata.binstall.overrides.x86_64-pc-windows-msvc]
  pkg-fmt = "zip"

  [package.metadata.binstall.overrides.aarch64-pc-windows-msvc]
  pkg-fmt = "zip"
  ```
  - Uses `.tgz` for Unix, `.zip` for Windows (platform convention)
  - Binary name handled automatically by Cargo (adds `.exe` on Windows)
  - No hardcoded executable names
- **Recommendation:** No changes needed
- **Priority:** N/A

### File I/O - Excellent

- **Issue:** None
- **Platforms affected:** All
- **Location:** `src/cli/handlers/encode.rs:43,64`, `src/core/config.rs:218`
- **Evidence:**
  ```rust
  let metadata = fs::metadata(file_path)?;
  fs::read(file_path)?
  let content = std::fs::read_to_string(path)?;
  ```
  - Uses Rust standard library, which handles platform differences
  - Text mode vs binary mode handled correctly by Rust's `Read`/`Write` traits
  - No platform-specific file operations (chmod, symlinks, etc.)
- **Recommendation:** No changes needed
- **Priority:** N/A

### Dependencies - Excellent

- **Issue:** None
- **Platforms affected:** All
- **Location:** `Cargo.toml:19-48`
- **Evidence:**
  - **crossterm** (line 30) - cross-platform terminal manipulation
  - **dirs** (line 27) - cross-platform directory locations
  - **shellexpand** (line 47) - portable tilde expansion
  - All other dependencies are pure Rust or have platform-specific implementations
  - `cargo tree` shows appropriate `windows-sys` dependencies only on Windows
- **Recommendation:** No changes needed
- **Priority:** N/A

## Platform-Specific Code Audit

All platform-specific code is **intentional and correct**:

| Location | Purpose | Correct? |
|----------|---------|----------|
| `src/simd/mod.rs:7-77` | Architecture-specific SIMD imports | ✅ Yes |
| `src/simd/x86_64/` | x86_64 AVX2/SSSE3 implementations | ✅ Yes |
| `src/simd/aarch64/` | ARM NEON implementations | ✅ Yes |
| `src/bench.rs:59,62,75` | Architecture detection for benchmarking | ✅ Yes |
| `Cargo.toml:72-76` | Windows ZIP packaging override | ✅ Yes |

**No inappropriate platform-specific code found.**

## CI/CD Matrix

| Platform | Tested in CI | Notes |
|----------|--------------|-------|
| Linux x86_64 | ✅ Yes | ubuntu-latest, full test suite |
| macOS aarch64 | ✅ Yes | macos-latest (M1), full test suite |
| Windows x86_64 | ✅ Yes | windows-latest, full test suite |
| Linux aarch64 | ❌ No | Could add via QEMU or native ARM runner |
| macOS x86_64 | ❌ No | Could add macos-13 or earlier |
| Windows aarch64 | ❌ No | Limited CI support, but code exists |

CI uses reusable workflows from `coryzibell/nebuchadnezzar`, which provides:
- Parallel testing on all platforms
- Automatic version bumping on main
- Release binary building via `follow-the-white-rabbit.yml`
- Cargo publish via `knock-knock.yml`

## Recommendations

### Critical
None. Core functionality is portable.

### Improve
1. **Add `.gitattributes`** - Normalize line endings in source files
   - Priority: Medium
   - Prevents Windows developers from committing CRLF in Rust source
   - Does NOT affect the hardcoded `\r\n` for terminal output (that's correct)

2. **Test on Linux ARM** - Add CI for `aarch64-unknown-linux-gnu`
   - Priority: Medium
   - SIMD code is implemented but never tested on ARM Linux
   - Could use GitHub's ARM runners or QEMU cross-compilation

### Consider
1. **Add macOS x86_64 CI** - Test on Intel Macs
   - Priority: Low
   - Current aarch64 coverage is good, but x86_64 has different SIMD paths
   - Could use `macos-13` runner

2. **Document platform support** - Add platform matrix to README
   - Priority: Low
   - Users should know what's tested vs untested
   - Mention SIMD availability per platform

3. **Cross-compilation testing** - Use `cross` crate in CI
   - Priority: Low
   - Already mentioned in the code comments (`docs/SIMD.md` references)
   - Would validate more exotic targets (RISC-V, etc.)

## What's Good

1. **Rust's standard library does heavy lifting** - Path handling, file I/O, all portable by default
2. **Mature dependencies** - `dirs`, `crossterm`, `shellexpand` are battle-tested cross-platform
3. **SIMD architecture gating is exemplary** - Clear separation, runtime detection, graceful fallback
4. **CI tests all major platforms** - Catches platform-specific issues before release
5. **No shell scripts in repo** - Everything is Rust or GitHub Actions YAML (portable)
6. **Config directory handling** - Respects platform conventions via `dirs` crate
7. **No filesystem permission assumptions** - Doesn't try chmod, symlinks, etc.
8. **Binary packaging configured** - Windows gets `.zip`, Unix gets `.tgz`
9. **Terminal output uses CRLF intentionally** - Correct for cross-platform cursor control
10. **No hardcoded paths** - All paths use `PathBuf::join()` or platform-aware abstractions

---

**Down here, I make the rules. And the rule is: Rust's cross-platform story is solid. This code respects it.**

---

[Identity: Trainman | Model: sonnet | Status: success]

Knock knock, Neo.
