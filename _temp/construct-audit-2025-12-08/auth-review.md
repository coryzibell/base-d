# Auth Review: base-d

## Summary

base-d is a CLI encoding/hashing tool with **no authentication or authorization features**. This is appropriate for its use case as a local utility. The review focuses on cryptographic implementations, secret handling for xxHash, and data integrity patterns.

**Overall Security Assessment:** Low Risk
The tool handles no credentials, sessions, or user accounts. Primary concerns are around hash algorithm usage, secret file handling, and input validation.

## Auth Model

- **Type:** N/A - Local CLI utility
- **Factors:** N/A
- **AAL Level:** N/A

This tool does not implement authentication. Users interact directly via CLI without any access control mechanisms.

## OWASP ASVS Assessment

| Section | Status | Notes |
|---------|--------|-------|
| V2 Authentication | N/A | No authentication features |
| V3 Session Management | N/A | No session management |
| V4 Access Control | N/A | No access control beyond filesystem permissions |
| V6 Cryptography | ⚠️ Partial | Uses cryptographic hashing, see findings below |

## Scope of Review

Since this is a CLI encoding tool without authentication features, the review focuses on:

1. **Cryptographic hash implementation** - 26 algorithms via RustCrypto
2. **Secret handling** - xxHash3 secret file loading
3. **Key derivation** - None present
4. **Input validation** - Path traversal protection
5. **Credential storage** - None present

---

## Findings

### 1. MD5 Algorithm Available

**Location:** `/home/kautau/work/personal/code/base-d/src/features/hashing.rs:47,225`

**Issue:** MD5 is exposed as a hashing option despite being cryptographically broken since 2004.

**Risk:** Users may select MD5 for security-critical operations without understanding it's unsuitable for cryptographic verification. Collision attacks are practical.

**Evidence:**
```rust
HashAlgorithm::Md5,  // Line 47
// ...
HashAlgorithm::Md5 => {  // Line 225
    let mut hasher = Md5::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
```

**ASVS Ref:** V6.2.1 - All cryptographic modules shall use approved cryptographic algorithms

**Recommendation:**
- Mark MD5 with clear deprecation warning in CLI help text
- Print stderr warning when MD5 is used: `"Warning: MD5 is cryptographically broken. Use SHA-256, BLAKE3, or SHA-3 instead."`
- Consider requiring explicit `--allow-insecure-md5` flag

**Priority:** Medium

---

### 2. xxHash3 Secret File Path Validation

**Location:** `/home/kautau/work/personal/code/base-d/src/cli/config.rs:6-28`

**Issue:** Path traversal protection for secret files is well-implemented.

**What's Good:**
```rust
fn validate_config_path(path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let expanded = shellexpand::tilde(path);
    let canonical = fs::canonicalize(expanded.as_ref())
        .map_err(|e| format!("Cannot access path '{}': {}", path, e))?;

    let allowed_base = dirs::config_dir()
        .ok_or("Cannot determine config directory")?
        .join("base-d");

    if !canonical.starts_with(&allowed_base) {
        return Err(format!(
            "Path '{}' escapes allowed directory. Files must be within ~/.config/base-d/",
            path
        )
        .into());
    }

    Ok(canonical)
}
```

This correctly prevents directory traversal attacks by:
1. Expanding tilde notation
2. Canonicalizing the path (follows symlinks, resolves `.` and `..`)
3. Validating it's within `~/.config/base-d/`

**Recommendation:** None - this is well done.

**Priority:** N/A (Positive finding)

---

### 3. xxHash3 Secret Size Validation

**Location:** `/home/kautau/work/personal/code/base-d/src/features/hashing.rs:30-42`

**Issue:** Secret validation enforces minimum size requirement.

**What's Good:**
```rust
pub fn with_secret(seed: u64, secret: Vec<u8>) -> Result<Self, String> {
    if secret.len() < 136 {
        return Err(format!(
            "XXH3 secret must be >= 136 bytes, got {}",
            secret.len()
        ));
    }
    Ok(Self {
        seed,
        secret: Some(secret),
    })
}
```

The 136-byte minimum is correct per xxHash3 specification. Secret entropy is critical for keyed hashing.

**Recommendation:** None - correctly implemented.

**Priority:** N/A (Positive finding)

---

### 4. Secret Material in stdin

**Location:** `/home/kautau/work/personal/code/base-d/src/cli/config.rs:118-121`

**Issue:** Secrets can be provided via stdin with `--xxhash-secret-stdin` or `--secret-stdin` flags.

**Risk:** Minimal - this is appropriate for a CLI tool. Stdin is commonly used for secret injection in Unix pipelines.

**Pattern:**
```rust
let secret = if cli_xxhash_secret_stdin {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf)?;
    Some(buf)
} // ...
```

**What's Good:**
- Secret is not logged
- Secret is not written to disk
- Memory is not explicitly zeroed, but Rust's ownership ensures it's dropped after use

**Consideration:** Secret material remains in process memory until garbage collection. For highly sensitive operations, consider using `zeroize` crate.

**Recommendation:** Document that secrets in memory are not zeroed. For most use cases (non-cryptographic xxHash), this is acceptable.

**Priority:** Low

---

### 5. No Key Derivation Functions

**Location:** N/A

**Issue:** No password-based key derivation (PBKDF2, Argon2, scrypt) is present.

**Risk:** None - this tool doesn't store or verify passwords.

**Observation:** If future features require password-to-key derivation, use Argon2id.

**Priority:** N/A

---

### 6. Hash Algorithm Security Tiers

**Location:** `/home/kautau/work/personal/code/base-d/src/features/hashing.rs:44-76`

**Issue:** No distinction between cryptographic and non-cryptographic hashes in CLI help.

**Risk:** Users may use CRC32 or xxHash for integrity verification where cryptographic hashes are needed.

**Current State:**
```rust
pub enum HashAlgorithm {
    Md5,           // Broken
    Sha256,        // Cryptographic
    Blake3,        // Cryptographic
    Crc32,         // Non-cryptographic (checksum)
    XxHash64,      // Non-cryptographic (speed)
    Ascon,         // Cryptographic (lightweight)
    K12,           // Cryptographic (XOF)
}
```

**Recommendation:**
- Document hash tiers in CLI help:
  - **Cryptographic:** SHA-256, SHA-3, BLAKE2, BLAKE3, Ascon, K12, Keccak
  - **Legacy (insecure):** MD5
  - **Non-cryptographic:** CRC16/32/64, xxHash
- Add `--purpose` flag with options: `integrity`, `checksum`, `speed`
- Suggest appropriate algorithms based on purpose

**Priority:** Medium

---

### 7. No Constant-Time Operations

**Location:** Hash comparison (if implemented)

**Issue:** Hash comparisons are not constant-time.

**Risk:** Minimal - this is a CLI tool, not a verification server. Timing attacks require repeated oracle queries.

**Observation:** If verification features are added (e.g., `base-d verify --hash sha256 <expected>`), use constant-time comparison via `subtle` crate.

**Priority:** Low

---

## Cryptography Audit

| Usage | Algorithm | Key Size | Status |
|-------|-----------|----------|--------|
| Hashing | MD5 | N/A | ⚠️ Broken - warn users |
| Hashing | SHA-224/256/384/512 | N/A | ✅ Standard cryptographic hashes |
| Hashing | SHA3-224/256/384/512 | N/A | ✅ Modern NIST standard |
| Hashing | Keccak-256 | N/A | ✅ Ethereum/blockchain use |
| Hashing | BLAKE2b/2s | N/A | ✅ High performance |
| Hashing | BLAKE3 | N/A | ✅ Fastest modern hash |
| Hashing | Ascon | N/A | ✅ Lightweight (IoT) |
| Hashing | KangarooTwelve | N/A | ✅ XOF capability |
| Checksum | CRC16/32/32c/64 | N/A | ✅ Non-crypto checksums |
| Checksum | xxHash32/64/3 | 64-bit seed + 136B secret | ✅ Non-crypto, correct secret size |

**All implementations via RustCrypto** - well-audited, pure Rust (no OpenSSL dependency).

---

## Secrets Inventory

| Secret Type | Storage | Rotation | Logged? |
|-------------|---------|----------|---------|
| xxHash3 secret | File (`~/.config/base-d/*.bin`) or stdin | Manual (user managed) | ❌ No |
| xxHash seed | Config file or CLI flag | Manual (user managed) | ❌ No |

**What's Good:**
- Secrets are not logged to stdout/stderr
- Path validation prevents traversal attacks
- Secrets loaded from well-defined config directory

**Consideration:**
- No automatic secret rotation
- Secrets stored in plaintext files (acceptable for xxHash use case)
- User responsible for filesystem permissions on secret files

---

## Recommendations

### Medium Priority

1. **MD5 Deprecation Warning**
   - Print stderr warning when MD5 is selected
   - Update docs to mark MD5 as "legacy/insecure"
   - Suggest SHA-256 or BLAKE3 as alternatives

2. **Hash Algorithm Documentation**
   - Clearly distinguish cryptographic vs non-cryptographic hashes
   - Add usage guidance: "Use SHA-256 for integrity, xxHash for speed, CRC for error detection"
   - Document threat models for each algorithm class

### Low Priority

3. **Memory Zeroing for Secrets**
   - Consider `zeroize` crate for xxHash3 secrets
   - Document that secrets are not zeroed from memory
   - Acceptable for current use case, but future-proofing

4. **Secret File Permissions Check**
   - Warn if secret file is world-readable
   - Suggest `chmod 600 ~/.config/base-d/xxh3-secret.bin`

5. **Constant-Time Comparison**
   - If verification features are added, use `subtle::ConstantTimeEq`
   - Current tool has no comparison operations

---

## What's Good

### Cryptographic Hygiene

1. **Modern Algorithm Selection**
   - Excellent range: SHA-3, BLAKE3, Ascon, K12
   - All via RustCrypto (well-audited, maintained)
   - No OpenSSL dependency (eliminates entire class of supply chain risks)

2. **Path Traversal Protection**
   - Robust validation in `validate_config_path()`
   - Canonical path checking
   - Scoped to config directory

3. **Secret Handling**
   - Correct xxHash3 secret size enforcement (136 bytes)
   - Secrets not logged or printed
   - Clean separation of cryptographic and non-cryptographic use

4. **Input Size Limits**
   - `--max-size` flag prevents memory exhaustion
   - Default limits on stdin input

5. **Pure Rust Implementation**
   - No C bindings to OpenSSL/libcrypto
   - Fewer supply chain attack vectors
   - Better memory safety

---

## Security Anti-Patterns: None Found

The codebase avoids common auth/crypto anti-patterns:
- ✅ No plaintext password storage (N/A - no passwords)
- ✅ No weak token generation (N/A - no tokens)
- ✅ No hardcoded secrets
- ✅ No SQL injection (N/A - no database)
- ✅ No command injection (validated paths only)
- ✅ No predictable secret generation

---

## Configuration Security

**Config file location:** `~/.config/base-d/dictionaries.toml`

**Settings reviewed:**
```toml
[settings.xxhash]
default_seed = 0
default_secret_file = "~/.config/base-d/xxh3-secret.bin"
```

**What's Good:**
- Config in standard XDG location
- Tilde expansion handled correctly
- No sensitive defaults

**Recommendation:**
- Document filesystem permissions for config directory
- Suggest `chmod 700 ~/.config/base-d/` for users storing secrets

---

## Test Coverage

**Location:** `/home/kautau/work/personal/code/base-d/src/features/hashing.rs:376-616`

**What's Good:**
- Comprehensive test suite for all hash algorithms
- Known-answer tests (e.g., MD5 of "hello world" matches expected)
- Edge case testing (empty input, seed variations)
- Secret validation testing

**Example:**
```rust
#[test]
fn test_xxhash_config_secret_too_short() {
    let result = XxHashConfig::with_secret(0, vec![0u8; 100]);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("136 bytes"));
}
```

**Recommendation:** None - testing is solid.

---

## Threat Model

### In Scope
- Local file access (mitigated by path validation)
- Hash collision attacks (MD5 vulnerable)
- Secret file permissions (user responsibility)

### Out of Scope
- Network attacks (no network functionality)
- Authentication bypass (no authentication)
- Session hijacking (no sessions)
- SQL injection (no database)

### Residual Risks
1. **User selects MD5 for security purposes** - Medium (addressed by warnings)
2. **Secret files have incorrect permissions** - Low (user education)
3. **Secrets remain in process memory** - Low (acceptable for xxHash)

---

## Compliance Notes

**NIST SP 800-63B:** N/A - no authentication
**OWASP ASVS v4.0:** N/A for auth, partial for crypto
**CWE-327 (Broken Crypto):** MD5 present but non-default

---

**Knock knock, Neo.**
