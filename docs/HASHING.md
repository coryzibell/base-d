# Hashing Support

base-d includes built-in support for multiple hash algorithms including cryptographic hashes (RustCrypto), CRC checksums, and xxHash for non-cryptographic use cases. All algorithms are implemented in pure Rust without any C dependencies or OpenSSL linking requirements.

## Supported Algorithms

### Cryptographic Hashes

| Algorithm | Flag | Output Size | Best For |
|-----------|------|-------------|----------|
| **MD5** | `--hash md5` | 128 bits (16 bytes) | Legacy compatibility (NOT secure) |
| **SHA-224** | `--hash sha224` | 224 bits (28 bytes) | Truncated SHA-256 |
| **SHA-256** | `--hash sha256` | 256 bits (32 bytes) | General purpose, widely supported |
| **SHA-384** | `--hash sha384` | 384 bits (48 bytes) | Truncated SHA-512 |
| **SHA-512** | `--hash sha512` | 512 bits (64 bytes) | High security, large digests |
| **SHA3-224** | `--hash sha3-224` | 224 bits (28 bytes) | Modern NIST standard |
| **SHA3-256** | `--hash sha3-256` | 256 bits (32 bytes) | Modern NIST standard |
| **SHA3-384** | `--hash sha3-384` | 384 bits (48 bytes) | Modern NIST standard |
| **SHA3-512** | `--hash sha3-512` | 512 bits (64 bytes) | Modern NIST standard |
| **Keccak-224** | `--hash keccak224` | 224 bits (28 bytes) | Ethereum (pre-standardization) |
| **Keccak-256** | `--hash keccak256` | 256 bits (32 bytes) | Ethereum, blockchain |
| **Keccak-384** | `--hash keccak384` | 384 bits (48 bytes) | Ethereum variant |
| **Keccak-512** | `--hash keccak512` | 512 bits (64 bytes) | Ethereum variant |
| **BLAKE2b** | `--hash blake2b` | 512 bits (64 bytes) | Fastest, high security |
| **BLAKE2s** | `--hash blake2s` | 256 bits (32 bytes) | Fast, optimized for 32-bit |
| **BLAKE3** | `--hash blake3` | 256 bits (32 bytes) | Fastest modern hash |

### CRC Checksums (Non-Cryptographic)

| Algorithm | Flag | Output Size | Best For |
|-----------|------|-------------|----------|
| **CRC-16** | `--hash crc16` | 16 bits (2 bytes) | Simple error detection |
| **CRC-32** | `--hash crc32` | 32 bits (4 bytes) | ZIP, Ethernet, PNG files |
| **CRC-32C** | `--hash crc32c` | 32 bits (4 bytes) | iSCSI, Btrfs (Castagnoli) |
| **CRC-64** | `--hash crc64` | 64 bits (8 bytes) | Large data integrity |

### xxHash (Ultra-Fast Non-Cryptographic)

| Algorithm | Flag | Output Size | Best For |
|-----------|------|-------------|----------|
| **xxHash32** | `--hash xxhash32` | 32 bits (4 bytes) | Hash tables, cache keys |
| **xxHash64** | `--hash xxhash64` | 64 bits (8 bytes) | Fast checksums, deduplication |

## Basic Usage

### Compute Hash (Hex Output)

```bash
# SHA-256 hash (default hex output)
echo "hello world" | base-d --hash sha256
# Output: a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447

# MD5 hash
echo "hello world" | base-d --hash md5
# Output: 6f5902ac237024bdd0c176cb93063dc4

# BLAKE3 hash (fastest)
echo "hello world" | base-d --hash blake3
# Output: dc5a4edb8240b018124052c330270696f96771a63b45250a5c17d3000e823355

# CRC32 checksum
echo "hello world" | base-d --hash crc32
# Output: af083b2d

# xxHash64 (ultra-fast non-cryptographic)
echo "hello world" | base-d --hash xxhash64
# Output: 5215e13b207d6d8c
```

### Hash with Custom Encoding

```bash
# SHA-256 encoded as base64
echo "hello world" | base-d --hash sha256 -e base64
# Output: qUiQTy8PR5uPgZdpSzAYSw0u0cHNKh7A+4XSmaGSpEc=

# BLAKE3 encoded as base85
echo "hello world" | base-d --hash blake3 -e base85
# Output: uXS@`SLKR5BkO(4w!p`kMvC`POkD?=_gFqkEyZ@e

# SHA-512 encoded as emoji
echo "hello world" | base-d --hash sha512 -e emoji_faces

# CRC32C encoded as base64
echo "hello world" | base-d --hash crc32c -e base64
# Output: 8P9ykg==
```

### Hash Files

```bash
# Hash a file
base-d --hash sha256 document.txt

# Hash large files efficiently
base-d --hash blake3 large_file.iso

# Hash and encode with custom dictionary
base-d --hash sha256 -e base64 myfile.bin
```

### Pipeline Integration

```bash
# Decode, then hash
echo "SGVsbG8gd29ybGQ=" | base-d -d base64 --hash sha256

# Hash and then compress result (hash then encode compressed hash)
echo "data" | base-d --hash sha512 | base-d --compress gzip -e base64
```

## Algorithm Comparison

### Security Recommendations

- ✅ **Recommended**: SHA-256, SHA-512, SHA3-*, BLAKE2*, BLAKE3
- ⚠️ **Legacy/Specific Use**: SHA-224, SHA-384, Keccak-* (Ethereum)
- ❌ **NOT Secure**: MD5 (collisions known, use only for checksums)

### Performance Characteristics

**Cryptographic Hashes** (Relative speeds on modern hardware):

1. **BLAKE3**: ~1000 MB/s (fastest, parallelized)
2. **BLAKE2b**: ~800 MB/s
3. **BLAKE2s**: ~700 MB/s
4. **MD5**: ~600 MB/s (not secure)
5. **SHA-512**: ~500 MB/s (faster than SHA-256 on 64-bit)
6. **SHA-256**: ~300 MB/s
7. **SHA3-256**: ~150 MB/s
8. **Keccak-256**: ~150 MB/s

**Non-Cryptographic** (Much faster):

1. **xxHash64**: ~15 GB/s (ultra-fast)
2. **xxHash32**: ~12 GB/s
3. **CRC32C** (hardware): ~10 GB/s
4. **CRC32**: ~1 GB/s

### Use Case Guide

| Use Case | Recommended Algorithm |
|----------|----------------------|
| **Cryptographic** | |
| General checksums | SHA-256, BLAKE3 |
| High-speed checksums | BLAKE3, BLAKE2b |
| Cryptographic signatures | SHA-256, SHA-512 |
| File integrity (secure) | SHA-256, BLAKE2b |
| Ethereum/blockchain | Keccak-256 |
| **Non-Cryptographic** | |
| File integrity (fast) | CRC32, CRC32C |
| ZIP/PNG compatibility | CRC32 |
| Hash tables | xxHash32, xxHash64 |
| Data deduplication | xxHash64, CRC64 |
| Cache keys | xxHash32, xxHash64 |
| Legacy compatibility | MD5, CRC16 |

### When to Use What

- **Use cryptographic hashes** when you need security (tamper resistance, collision resistance)
- **Use CRC** for error detection in files/networks (ZIP, Ethernet)
- **Use xxHash** for maximum speed when security isn't needed (caching, deduplication)

## Pure Rust Implementation

All hash algorithms are implemented in **pure Rust** with:

- ✅ No C dependencies
- ✅ No OpenSSL linking
- ✅ Cross-platform compilation
- ✅ Memory-safe by design
- ✅ Constant-time operations where applicable (cryptographic)

### Libraries Used

**Cryptographic**:
- `sha2` - SHA-224, SHA-256, SHA-384, SHA-512
- `sha3` - SHA3 family and Keccak variants
- `blake2` - BLAKE2b and BLAKE2s
- `blake3` - BLAKE3 (Rust-first design)
- `md-5` - MD5 (legacy)

**Non-Cryptographic**:
- `crc` - CRC16, CRC32, CRC32C, CRC64
- `twox-hash` - xxHash32, xxHash64

## Library API

```rust
use base_d::{HashAlgorithm, hash};

// Compute hash
let data = b"hello world";
let hash_output = hash(data, HashAlgorithm::Sha256);

// Output size
let size = HashAlgorithm::Sha256.output_size(); // 32 bytes

// Parse from string
let algo = HashAlgorithm::from_str("sha256")?;
```

## Advanced Examples

### Verify File Integrity

```bash
# Create checksum
base-d --hash sha256 file.zip > file.zip.sha256

# Verify later
base-d --hash sha256 file.zip | diff - file.zip.sha256
```

### Multi-Algorithm Verification

```bash
# Generate multiple checksums
echo "data" | base-d --hash md5 > file.md5
echo "data" | base-d --hash sha256 > file.sha256
echo "data" | base-d --hash blake3 > file.blake3
```

### Encoded Hash Storage

```bash
# Store hash in base64 (more compact)
base-d --hash sha512 document.pdf -e base64 > document.pdf.hash

# Store in base85 (even more compact)
base-d --hash sha256 file.bin -e base85
```

## Implementation Notes

### Memory Usage
- All algorithms use constant memory regardless of input size
- Stream processing for large files
- No buffering of entire input

### Thread Safety
- All hash functions are thread-safe
- Can be called concurrently
- No shared mutable state

### Platform Support
- Works on all Rust-supported platforms
- No architecture-specific requirements
- ARM, x86, x86_64, RISC-V, etc.

## Error Handling

```bash
# Invalid algorithm name
base-d --hash invalid
# Error: Unknown hash algorithm: invalid

# Works with any input encoding
echo "Hello" | base-d --hash sha256
# Always produces consistent output
```

## Future Enhancements

Potential additions (see ROADMAP.md):
- HMAC support with keyed hashing
- Incremental hashing for streaming
- Hash comparison utilities
- Multi-threaded parallel hashing for large files
