# Security Findings: base-d

**Project:** base-d v3.0.17
**Type:** CLI tool + library (encoding/decoding)
**Reviewed:** 2025-12-08
**Reviewer:** Cypher

---

## Summary

The codebase demonstrates **solid security practices** overall. No critical vulnerabilities found. The primary concerns are decompression bomb risks and unconstrained memory allocation in decompression paths. SIMD unsafe code is properly bounds-checked. Input validation is thorough for encoding/decoding operations. Path traversal protection is implemented for config files.

**Security Posture:** Good with minor improvements needed.

---

## Threat Model

- **Project type:** CLI tool + library
- **Trust boundaries:**
  - User-provided file paths (encode/decode input/output)
  - User-provided dictionary names (indirect code execution if compromised config)
  - Network/untrusted encoded data (decode operations)
  - Config files (`~/.config/base-d/`)
- **Sensitive data:**
  - xxHash secrets (optional, user-provided)
  - Temporary data during encode/decode (not persisted)
- **Attack surface:**
  - File I/O operations
  - Schema deserialization (JSON, binary unpacking)
  - Compression/decompression
  - SIMD intrinsics (unsafe code)
  - Config file loading (path traversal risk)

---

## Critical Findings

**None.**

---

## High Findings

### H1: Decompression Bomb (Zip Bomb) Risk

**Severity:** High
**Category:** OWASP A05 - Security Misconfiguration / Denial of Service
**Location:**
- `/home/kautau/work/personal/code/base-d/src/features/compression.rs:112-179`
- Specifically: `decompress_gzip()`, `decompress_zstd()`, `decompress_brotli()`, `decompress_lzma()`

**Description:**

Most decompression functions use unbounded `read_to_end()` which allows attackers to craft malicious compressed data that expands to gigabytes/terabytes of uncompressed data, exhausting memory.

```rust
fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use flate2::read::GzDecoder;
    let mut decoder = GzDecoder::new(data);
    let mut result = Vec::new();
    decoder.read_to_end(&mut result)?;  // ⚠️ UNBOUNDED
    Ok(result)
}
```

Only LZ4 has a hard limit (100MB):

```rust
fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(lz4::block::decompress(data, Some(100 * 1024 * 1024))?)  // ✅ 100MB limit
}
```

**Proof of Concept:**

A 10KB gzip bomb can expand to 10GB:

```bash
# Create a 10GB file of zeros compressed to ~10KB
dd if=/dev/zero bs=1M count=10240 | gzip -9 > bomb.gz
base-d decode --file bomb.gz --decompress gzip --dictionary base64
# Memory exhaustion, OOM killer triggers
```

**Impact:**
- Denial of Service (memory exhaustion)
- System crash (OOM)
- Affects both CLI and library users

**Recommendation:**

1. Add configurable decompression limits (default 100MB, CLI flag `--max-decompress-size`)
2. Implement streaming decompression with read limits
3. Add early detection: track expansion ratio (if `output_size / input_size > 1000`, abort)

Example fix:

```rust
const MAX_DECOMPRESS_SIZE: usize = 100 * 1024 * 1024; // 100MB

fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use flate2::read::GzDecoder;
    let mut decoder = GzDecoder::new(data);
    let mut result = Vec::new();

    // Read with limit
    decoder.take(MAX_DECOMPRESS_SIZE as u64).read_to_end(&mut result)?;

    // Check if we hit the limit (potential bomb)
    if result.len() >= MAX_DECOMPRESS_SIZE {
        return Err("Decompression limit exceeded (potential bomb)".into());
    }
    Ok(result)
}
```

**References:**
- [CWE-409: Improper Handling of Highly Compressed Data (Zip Bomb)](https://cwe.mitre.org/data/definitions/409.html)
- [OWASP: Denial of Service](https://owasp.org/www-community/attacks/Denial_of_Service)

---

## Medium Findings

### M1: Path Traversal Protection Only After Canonicalization

**Severity:** Medium
**Category:** OWASP A01 - Broken Access Control
**Location:** `/home/kautau/work/personal/code/base-d/src/cli/config.rs:10-28`

**Description:**

The `validate_config_path()` function protects against path traversal, but only **after** canonicalization. If the file doesn't exist, canonicalization fails and the check is bypassed.

```rust
fn validate_config_path(path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let expanded = shellexpand::tilde(path);
    let canonical = fs::canonicalize(expanded.as_ref())
        .map_err(|e| format!("Cannot access path '{}': {}", path, e))?;  // ⚠️ Fails if file doesn't exist

    let allowed_base = dirs::config_dir()
        .ok_or("Cannot determine config directory")?
        .join("base-d");

    if !canonical.starts_with(&allowed_base) {
        return Err(format!("Path '{}' escapes allowed directory...", path).into());
    }
    Ok(canonical)
}
```

**Impact:**

If an attacker can trigger file creation (e.g., via symlink race), they could potentially write outside the allowed directory. However, the current usage only **reads** files (xxHash secret file), limiting exploitability.

Current usage:
```rust
// cli/config.rs:123
} else if let Some(ref path) = config.settings.xxhash.default_secret_file {
    let validated_path = validate_config_path(path)?;
    Some(fs::read(validated_path)?)  // Only reads, doesn't write
}
```

**Recommendation:**

1. Validate **before** canonicalization by checking for `..` components
2. If file doesn't exist, validate parent directory instead

```rust
fn validate_config_path(path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let expanded = shellexpand::tilde(path);
    let path_buf = PathBuf::from(expanded.as_ref());

    // Check for path traversal attempts before canonicalization
    if path_buf.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("Path contains '..' - potential traversal attempt".into());
    }

    // Try to canonicalize, fall back to parent if file doesn't exist
    let canonical = if path_buf.exists() {
        fs::canonicalize(&path_buf)?
    } else {
        // Validate parent directory instead
        let parent = path_buf.parent().ok_or("Invalid path")?;
        fs::canonicalize(parent)?.join(path_buf.file_name().ok_or("Invalid filename")?)
    };

    let allowed_base = dirs::config_dir()
        .ok_or("Cannot determine config directory")?
        .join("base-d");

    if !canonical.starts_with(&allowed_base) {
        return Err(format!("Path '{}' escapes allowed directory", path).into());
    }
    Ok(canonical)
}
```

**References:**
- [CWE-22: Path Traversal](https://cwe.mitre.org/data/definitions/22.html)
- [OWASP: Path Traversal](https://owasp.org/www-community/attacks/Path_Traversal)

---

### M2: VarInt Overflow Protection Present But Could Be Stricter

**Severity:** Medium (Informational - Already Protected)
**Category:** OWASP A03 - Injection / Data Validation
**Location:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/binary_unpacker.rs:300-323`

**Description:**

The VarInt decoder protects against overflow by limiting to 64 bits (10 bytes max), which is correct for LEB128:

```rust
fn decode_varint(cursor: &mut Cursor, context: &str) -> Result<u64, SchemaError> {
    let start_pos = cursor.pos;
    let mut result = 0u64;
    let mut shift = 0;

    loop {
        if shift >= 64 {  // ✅ Prevents overflow
            return Err(SchemaError::InvalidVarint {
                context: context.to_string(),
                position: start_pos,
            });
        }

        let byte = cursor.read_byte()?;
        result |= ((byte & 0x7F) as u64) << shift;
        shift += 7;

        if byte & 0x80 == 0 {
            break;
        }
    }

    Ok(result)
}
```

However, when varints are cast to `usize` for allocations, large values could still cause issues:

```rust
// binary_unpacker.rs:66
let len = decode_varint(cursor, "root key length")? as usize;  // ⚠️ Cast to usize
let bytes = cursor.read_bytes(len)?;  // Could request huge allocation
```

**Impact:**

On 64-bit systems, a varint encoding `u64::MAX` would be cast to `usize::MAX`, potentially causing:
- Massive memory allocation attempts
- Integer overflow in `read_bytes()` length calculation
- Denial of service

**Recommendation:**

Add reasonable limits when converting varint to `usize` for allocations:

```rust
const MAX_STRING_LENGTH: u64 = 100 * 1024 * 1024; // 100MB
const MAX_FIELD_COUNT: u64 = 100_000;
const MAX_ROW_COUNT: u64 = 10_000_000;

// Replace:
let len = decode_varint(cursor, "root key length")? as usize;

// With:
let len_u64 = decode_varint(cursor, "root key length")?;
if len_u64 > MAX_STRING_LENGTH {
    return Err(SchemaError::ExcessiveLength {
        context: "root key length".to_string(),
        value: len_u64,
        max: MAX_STRING_LENGTH,
    });
}
let len = len_u64 as usize;
```

**References:**
- [CWE-190: Integer Overflow](https://cwe.mitre.org/data/definitions/190.html)
- [CWE-770: Allocation of Resources Without Limits](https://cwe.mitre.org/data/definitions/770.html)

---

### M3: SIMD Unsafe Code Relies on Debug Assertions for Bounds Checks

**Severity:** Medium (Informational - Mitigated)
**Category:** Memory Safety
**Location:**
- `/home/kautau/work/personal/code/base-d/src/simd/lut/gapped.rs:268-284`
- Similar patterns in `base32.rs`, `base64.rs`

**Description:**

SIMD encoding uses `get_unchecked()` for performance, with bounds safety relying on `debug_assert!()`:

```rust
for _ in 0..num_blocks {
    debug_assert!(  // ⚠️ Only active in debug builds
        offset + 5 <= data.len(),
        "SIMD bounds check: offset {} + 5 exceeds len {}",
        offset,
        data.len()
    );

    let (b0, b1, b2, b3, b4) = unsafe {
        (
            *data.get_unchecked(offset),
            *data.get_unchecked(offset + 1),
            *data.get_unchecked(offset + 2),
            *data.get_unchecked(offset + 3),
            *data.get_unchecked(offset + 4),
        )
    };
    // ...
}
```

**Impact:**

If the `num_blocks` calculation is incorrect (e.g., due to logic error), **release builds** would have no bounds check, leading to:
- Out-of-bounds read (information disclosure)
- Potential segmentation fault (DoS)
- Undefined behavior

Current mitigation:
```rust
let num_blocks = data.len() / BLOCK_SIZE;  // Integer division ensures safety
let simd_bytes = num_blocks * BLOCK_SIZE;
```

This is **safe** because `num_blocks * BLOCK_SIZE <= data.len()` by construction. However, future refactoring could break this invariant.

**Recommendation:**

1. Add **explicit comments** documenting the safety invariant
2. Consider using `assert!()` instead of `debug_assert!()` for memory safety (small performance cost acceptable)
3. Add fuzzing tests to verify bounds safety under all inputs

```rust
// SAFETY INVARIANT: num_blocks is computed as data.len() / BLOCK_SIZE,
// so offset + BLOCK_SIZE will never exceed data.len() within the loop.
let num_blocks = data.len() / BLOCK_SIZE;

for _ in 0..num_blocks {
    // Use assert! for memory safety (not just debug_assert!)
    assert!(
        offset + BLOCK_SIZE <= data.len(),
        "SIMD bounds violation: offset {} + {} exceeds len {}",
        offset, BLOCK_SIZE, data.len()
    );
    // ... unsafe code
}
```

**References:**
- [Rust Nomicon: Unchecked Indexing](https://doc.rust-lang.org/nomicon/unchecked-uninit.html)
- [CWE-125: Out-of-bounds Read](https://cwe.mitre.org/data/definitions/125.html)

---

## Low Findings

### L1: No Rate Limiting for Repeated Decode Attempts

**Severity:** Low
**Category:** OWASP A09 - Security Logging & Monitoring Failures / DoS
**Location:** CLI handlers

**Description:**

The CLI allows unlimited decode attempts. An attacker could repeatedly attempt to decode malicious payloads to:
- Trigger decompression bombs (if H1 is exploited)
- Cause CPU exhaustion via expensive operations
- Fill logs with error messages

**Impact:** Minor DoS risk in automated/server contexts.

**Recommendation:**

Not critical for a CLI tool, but if this library is used in a server context:
- Add per-IP rate limiting in wrapper applications
- Implement exponential backoff for repeated failures
- Add telemetry/monitoring for decode errors

---

### L2: Control Character Detection Could Be Bypassed

**Severity:** Low
**Category:** Output Validation
**Location:** `/home/kautau/work/personal/code/base-d/src/cli/handlers/encode.rs:152-155`

**Description:**

The encode handler checks for control characters before outputting to stdout:

```rust
fn contains_control_chars(s: &str) -> bool {
    s.bytes()
        .any(|b| b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r')
}
```

However, this only checks bytes `< 0x20`. Other control characters exist in Unicode (e.g., C1 control characters `0x80-0x9F`, zero-width characters).

**Impact:**

Terminal injection attacks are mitigated but not fully prevented. An attacker could craft a dictionary with Unicode control characters to manipulate terminal output.

**Recommendation:**

Expand check to include:
- C1 control characters (`0x80-0x9F`)
- ANSI escape sequences (if not intended)
- Zero-width characters (if they could cause issues)

Or simply use `--output` flag to write to file instead of stdout.

---

### L3: Error Messages May Leak Sensitive Paths

**Severity:** Low
**Category:** OWASP A09 - Security Logging & Monitoring Failures / Information Disclosure
**Location:** Various error handlers

**Description:**

Error messages include full file paths, which could reveal directory structure:

```rust
.map_err(|e| format!("Cannot access path '{}': {}", path, e))?;
```

**Impact:** Minor information disclosure in multi-user systems.

**Recommendation:**

Sanitize paths in error messages (e.g., replace home directory with `~`).

---

## Dependency Audit

Attempted to run `cargo audit` but encountered environment issue (Nix-built binary incompatible with WSL2 glibc). Manual review of dependencies:

| Package | Version | Known Issues | Status |
|---------|---------|--------------|--------|
| rand | 0.9.2 | None found | ✅ |
| serde_json | 1.0.145 | None found | ✅ |
| markdown | 1.0.0-alpha.21 | ⚠️ Alpha version | Review |
| shellexpand | 3.1.1 | None found | ✅ |
| crossterm | 0.29.0 | None found | ✅ |
| brotli | 8.0.2 | None found | ✅ |
| zstd | 0.13.3 | None found | ✅ |
| lz4 | 1.28.1 | None found | ✅ |
| xz2 | 0.1.7 | None found | ✅ |

**Recommendation:** Pin `markdown` to a stable release when available, or vendor the alpha version with security review.

---

## OWASP Assessment

| Category | Status | Notes |
|----------|--------|-------|
| **A01: Broken Access Control** | ⚠️ Medium | Path traversal protection in place, minor edge case (M1) |
| **A02: Cryptographic Failures** | ✅ Pass | Uses well-known hash algorithms (SHA2/3, BLAKE2/3), no custom crypto |
| **A03: Injection** | ✅ Pass | No command execution, SQL, or code injection vectors. VarInt parsing protected. |
| **A04: Insecure Design** | ✅ Pass | Separation of concerns, proper error handling |
| **A05: Security Misconfiguration** | ⚠️ High | Decompression bomb risk (H1) - no limits on expansion |
| **A06: Vulnerable Components** | ⚠️ Low | `markdown` crate is alpha (L-tier risk), others are stable |
| **A07: Authentication Failures** | N/A | No authentication mechanism |
| **A08: Software & Data Integrity** | ✅ Pass | No CI/CD security issues detected, checksums via hashing feature |
| **A09: Security Logging Failures** | ⚠️ Low | No centralized logging, error messages may leak paths (L3) |
| **A10: Server-Side Request Forgery** | N/A | No URL fetching |

---

## STRIDE Threat Model Assessment

| Threat | Status | Notes |
|--------|--------|-------|
| **Spoofing** | ✅ Low | No identity/auth system to spoof |
| **Tampering** | ✅ Low | Hashing feature provides integrity verification |
| **Repudiation** | ⚠️ Medium | No audit logging of operations |
| **Information Disclosure** | ⚠️ Medium | Error messages leak paths (L3), SIMD OOB reads possible if bounds fail (M3) |
| **Denial of Service** | ⚠️ High | Decompression bombs (H1), varint memory exhaustion (M2) |
| **Elevation of Privilege** | ✅ Low | No privilege system |

---

## What's Good

Serious credit where it's due:

1. **Path Traversal Protection** - Proactive defense against config file attacks (`validate_config_path()`)
2. **VarInt Overflow Protection** - LEB128 decoder correctly limits to 64 bits
3. **Input Validation** - Decode operations validate characters against dictionary, proper error messages
4. **Memory Safety** - Rust's type system prevents most memory corruption
5. **SIMD Bounds Checking** - Debug assertions verify array access (though release builds rely on math)
6. **No Command Execution** - Zero `std::process::Command` usage, no shell escapes
7. **Controlled File I/O** - File operations use explicit paths, no wildcard expansions
8. **Clean Clippy** - Zero clippy warnings (ran with default lints)
9. **Control Character Detection** - Encode handler prevents terminal injection via control chars in stdout
10. **Limited LZ4 Decompression** - Only LZ4 has a hard 100MB limit (others should follow)

The SIMD code is particularly well-commented about safety invariants. The bounds arithmetic (`num_blocks = len / BLOCK_SIZE`) is sound.

---

## Recommended Fixes Priority

1. **High Priority:** Add decompression limits to all algorithms (H1)
2. **Medium Priority:** Strengthen path validation to handle non-existent files (M1)
3. **Medium Priority:** Add varint-to-usize conversion limits (M2)
4. **Low Priority:** Replace `debug_assert!` with `assert!` in SIMD bounds checks (M3)
5. **Low Priority:** Sanitize error message paths (L3)

---

## Test Recommendations

1. **Fuzzing:**
   - Fuzz decode operations with malformed input
   - Fuzz schema binary unpacker with crafted varints
   - Fuzz SIMD paths with edge-case lengths

2. **Decompression Bomb Tests:**
   - Create 10GB gzip bomb, verify rejection
   - Test expansion ratio detection

3. **Path Traversal Tests:**
   - Attempt `../../etc/passwd` in config file paths
   - Test symlink traversal

---

**Knock knock, Neo.**

[Identity: Cypher | Model: sonnet | Status: success]
