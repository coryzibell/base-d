# base-d: External Integrations Analysis

**Analyzed:** 2025-12-08
**Project:** base-d v3.0.17
**Location:** `/home/kautau/work/personal/code/base-d`

---

## Executive Summary

**Network Dependencies:** None
**External API Calls:** None
**File System Access:** Local only (stdin/stdout/files)
**Environment Variables:** Minimal (NO_COLOR, system architecture)
**Subprocess Execution:** None

This is a **pure local CLI tool** with no external service dependencies. All integrations are filesystem-based or local I/O.

---

## 1. Network & API Usage

### Finding: ZERO external network calls

**Searched for:**
- HTTP/HTTPS libraries: `reqwest`, `hyper`, `curl`, `http::`
- Network sockets: `std::net`, `TcpStream`, `UdpSocket`, `tokio::net`
- API patterns: `fetch`, `download`, `upload`, `webhook`

**Result:** No matches found in source code.

**Dependencies analysis (Cargo.toml):**
- No network libraries present
- All dependencies are computational/encoding libraries
- No async runtime (no tokio networking)

**Conclusion:** This tool operates entirely offline. No telemetry, no updates checks, no external API integrations.

---

## 2. File System Access

### 2.1 Configuration Files

**Location:** `~/.config/base-d/` (via `dirs::config_dir()`)

**Files accessed:**
- `~/.config/base-d/dictionaries.toml` - User-defined dictionary overrides
- `~/.config/base-d/<secret-file>` - Optional xxHash3 secret files

**Security measure:** Path validation in `src/cli/config.rs:validate_config_path()`
- Prevents path traversal attacks
- Ensures all file paths canonicalize within `~/.config/base-d/`
- Rejects paths that escape the allowed directory

```rust
// From src/cli/config.rs:10-28
fn validate_config_path(path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let expanded = shellexpand::tilde(path);
    let canonical = fs::canonicalize(expanded.as_ref())?;

    let allowed_base = dirs::config_dir()
        .ok_or("Cannot determine config directory")?
        .join("base-d");

    if !canonical.starts_with(&allowed_base) {
        return Err("Path escapes allowed directory".into());
    }
    Ok(canonical)
}
```

### 2.2 Dictionary Loading

**Priority cascade:**
1. Built-in dictionaries (embedded in binary via `include_str!`)
2. `~/.config/base-d/dictionaries.toml` (user overrides)
3. `./dictionaries.toml` (project-local overrides)

**Implementation:** `src/core/config.rs:load_with_overrides()`

### 2.3 I/O Operations

**Standard streams:**
- `stdin` - Read input data when no file specified
- `stdout` - Write encoded/decoded output
- `stderr` - Write hash outputs, warnings, errors

**File operations:**
- `fs::read_to_string()` - Read text input files
- `fs::read()` - Read binary files (for xxHash secrets)
- `fs::write()` - Write output files when `--output` specified
- `fs::File::open()` - Streaming mode for large files

**Pattern:** All file paths come from:
- CLI arguments (`--input`, `--output`, `--config`)
- Config file references (xxHash secret files)
- Never from network/external sources

---

## 3. Environment Variables

**Read access:**
- `NO_COLOR` - Disable ANSI color codes in error messages
- `std::env::consts::ARCH` - CPU architecture detection for SIMD path selection

**No write access** except in tests:
- Tests set/remove `NO_COLOR` for validation

**No sensitive variables accessed:**
- No `HOME`, `USER`, `PASSWORD`, `TOKEN`, etc.
- Uses `dirs` crate for platform-appropriate config paths

---

## 4. Subprocess Execution

**Finding:** Zero subprocess calls

**Searched for:**
- `Command::new`, `process::Command`, `spawn`, `exec`, `system`

**Only match:** `shellexpand::tilde()` in config validation
- This is **not** a subprocess call
- It's a pure Rust library that expands `~` to home directory
- No shell execution involved

---

## 5. External Dependencies (Cargo.toml)

### Computational Libraries (Safe)
- **Encoding:** `num-bigint`, `num-traits`, `num-integer` (math operations)
- **Serialization:** `serde`, `serde_json`, `toml` (config parsing)
- **Compression:** `flate2`, `brotli`, `zstd`, `lz4`, `snap`, `xz2`
- **Hashing:** `sha2`, `sha3`, `blake2`, `blake3`, `md-5`, `twox-hash`, `crc`, `ascon-hash`, `k12`

### CLI/UI Libraries (Safe)
- `clap` - Command-line argument parsing
- `terminal_size` - Detect terminal dimensions
- `crossterm` - Terminal control (colors, cursor positioning)

### System Libraries (Safe)
- `dirs` - Standard platform directories (`~/.config`, etc.)
- `shellexpand` - Tilde expansion (no shell execution)
- `rand` - Random dictionary selection
- `hex` - Hex encoding helpers
- `markdown` - Markdown parsing for schema docs

**No network-capable crates detected.**

---

## 6. Data Flow Contracts

### Input Sources
1. **stdin** - Raw binary/text data
2. **File path** (CLI arg) - User-specified input file
3. **Config files** - TOML configuration from standard locations

### Output Destinations
1. **stdout** - Encoded/decoded results
2. **stderr** - Hash outputs (when `--hash` flag used), warnings
3. **File path** (CLI arg) - User-specified output file

### Processing Pipeline
```
Input → [Optional: Compression] → Encoding/Decoding → [Optional: Hashing] → Output
```

### No external boundaries crossed:
- All processing happens in-memory or via local filesystem
- No IPC, no sockets, no shared memory with external processes
- No clipboard access, no GUI frameworks

---

## 7. Security Posture

### Strengths
✅ No network access - immune to network-based attacks
✅ Path traversal protection for config files
✅ No subprocess execution - no shell injection risk
✅ Pure Rust - memory safety guarantees
✅ No unsafe external FFI beyond standard compression libs

### Considerations
⚠️ **TOML parsing** - Config files parsed with `toml` crate (standard, well-tested)
⚠️ **Compression libraries** - Native Rust implementations (flate2, etc.) - C bindings for some
⚠️ **File permissions** - Relies on OS file permissions for config directory protection

### Threat Model
- **Local file access only** - Attacker would need local filesystem access
- **No remote code execution vectors** - No network, no subprocesses
- **Config tampering** - If attacker has write access to `~/.config/base-d/`, they can modify dictionaries
  - Mitigation: Standard OS file permissions
  - Impact: Limited to encoding behavior, not system compromise

---

## 8. Integration Points Summary

| Integration Type | Usage | Security |
|-----------------|-------|----------|
| Network/HTTP | ❌ None | N/A |
| External APIs | ❌ None | N/A |
| Subprocesses | ❌ None | N/A |
| File I/O | ✅ Local only | Path validation |
| Config files | ✅ TOML in `~/.config/base-d/` | Sandboxed to config dir |
| Environment vars | ✅ `NO_COLOR`, `ARCH` | Read-only, non-sensitive |
| Standard I/O | ✅ stdin/stdout/stderr | Standard streams |
| Clipboard | ❌ None | N/A |
| GUI/Display | ❌ None (terminal only) | N/A |

---

## 9. Recommendations

### For Library Users
- **Safe to embed** - No surprise network calls or subprocess execution
- **Predictable I/O** - All file operations explicit via config or API calls
- **Audit surface** - Small: config parsing + compression libs

### For CLI Users
- **Privacy-friendly** - No telemetry, no usage tracking, no network calls
- **Config control** - Override dictionaries without modifying binary
- **Standard locations** - Follows XDG/platform conventions via `dirs` crate

### For Security Review
- **Focus areas:**
  1. TOML parsing in `src/core/config.rs`
  2. Path validation in `src/cli/config.rs`
  3. Compression library usage (C FFI bindings)
- **Low risk profile** - Pure local tool, no external attack surface

---

## 10. External Service Integration Patterns

**Pattern:** None. This tool has no external service integrations.

**If adding in future:**
- Consider `reqwest` for HTTP (async)
- Add `--no-network` flag for paranoid users
- Document all network calls in README
- Make network features opt-in at compile time

---

## Appendix: Reference Locations

**Configuration loading:** `src/core/config.rs:load_with_overrides()`
**Path validation:** `src/cli/config.rs:validate_config_path()`
**File I/O handlers:** `src/cli/handlers/*.rs`
**Environment access:** `src/encoders/algorithms/errors.rs` (NO_COLOR check)
**Dependencies:** `Cargo.toml:19-48`

---

*End of analysis. This tool is a pure local CLI with zero external service dependencies.*
