# Copilot Instructions for base-d

## Project Overview

**base-d** is a universal multi-dictionary encoding library and CLI tool for Rust. It encodes binary data using 35+ built-in dictionaries including RFC standards (base64, base32), ancient scripts (hieroglyphs, cuneiform), emoji, playing cards, and more.

**Key Details:**
- Version: 0.1.18
- Language: Rust (edition 2021)
- License: MIT OR Apache-2.0
- Repository: https://github.com/coryzibell/base-d

## Core Architecture

### Three Encoding Modes

The project supports three distinct encoding strategies:

1. **BaseConversion** (Mathematical) - Treats binary data as a single large number and converts to target base. Works with any dictionary size. Variable output length.

2. **Chunked** (RFC 4648) - Processes data in fixed-size bit groups for RFC 4648 compatibility. Requires power-of-two dictionary sizes (2, 4, 8, 16, 32, 64, 128, 256). Supports padding character.

3. **ByteRange** - Direct 1:1 byte-to-character mapping using Unicode codepoint ranges. Always 256 characters, zero encoding overhead.

### Module Organization

```
src/
├── lib.rs              # Public API: encode(), decode(), hash()
├── main.rs             # CLI with clap parser
├── core/
│   ├── dictionary.rs   # Dictionary struct with fast ASCII lookup tables
│   └── config.rs       # TOML config, EncodingMode enum
├── encoders/
│   ├── encoding.rs     # Mathematical BigUint base conversion
│   ├── chunked.rs      # RFC 4648 bit-chunking (base64/32/16)
│   ├── byte_range.rs   # Direct byte-to-codepoint mapping
│   └── streaming.rs    # Memory-efficient large file processing
├── compression.rs      # 6 compression algorithms (gzip, zstd, brotli, lz4, snappy, lzma)
├── hashing.rs          # 24 hash algorithms (cryptographic, CRC, xxHash)
└── detection.rs        # Dictionary auto-detection from encoded input
```

### Key Data Types

- **Dictionary**: Main encoding dictionary containing character set, HashMap index, optional O(1) lookup table for ASCII, mode, and padding
- **DictionariesConfig**: Loads and manages dictionary definitions from TOML files
- **EncodingMode**: Enum determining which encoding algorithm to use
- **StreamingEncoder/Decoder**: Process large files with constant 4KB memory usage
- **CompressionAlgorithm**: Enum for 6 compression algorithms (pure Rust)
- **HashAlgorithm**: Enum for 24 hash algorithms (16 crypto + 4 CRC + 4 xxHash)

## Configuration System

Dictionaries are loaded with layered overrides:
1. Built-in from embedded `dictionaries.toml`
2. User-level: `~/.config/base-d/dictionaries.toml`
3. Project-level: `./dictionaries.toml`

Load via `DictionariesConfig::load_with_overrides()`.

## Performance Characteristics

Recent optimizations (see OPTIMIZATION_SUMMARY.md):
- Fast lookup tables: O(1) array-based ASCII lookup (5x improvement over HashMap)
- Pre-allocated buffers: Eliminate reallocation overhead during encoding
- Cache-friendly chunking: 64-byte chunks align with L1 cache lines
- Combined operations: Use `div_rem()` instead of separate `%` and `/`
- Benchmarks: Criterion.rs suite achieving ~370 MiB/s for base64 encoding

Performance targets:
- **Encoding**: Base64 ~370 MiB/s, base256_matrix 1:1 mapping (no overhead)
- **Compression**: Snappy ~600 MB/s, LZ4 ~500 MB/s, Gzip ~80 MB/s
- **Hashing**: xxHash3 ~30 GB/s, BLAKE3 ~1 GB/s, SHA-256 ~300 MB/s
- **Streaming**: Constant 4KB memory regardless of file size

## Recent Features (2024)

### Compression Support
- 6 algorithms: gzip, zstd, brotli, lz4, snappy, lzma
- Pure Rust implementations (no OpenSSL)
- CLI: `--compress <algo>`, `--decompress <algo>`, `--level <N>`
- Libraries: flate2, zstd, brotli, lz4, snap, xz2

### Hashing Support
- 24 algorithms total:
  - 16 cryptographic: MD5, SHA-2 family, SHA-3 family, Keccak, BLAKE2/3
  - 4 CRC checksums: CRC16, CRC32, CRC32C, CRC64
  - 4 xxHash: xxHash32, xxHash64, xxHash3-64, xxHash3-128
- Pure Rust implementations (RustCrypto, crc, twox-hash)
- CLI: `--hash <algo>` with optional encoding via `-e`
- Default hex output, or encode hash with any dictionary

### Dictionary Auto-Detection
- Automatically identify encoding dictionary from input
- CLI: `--detect` with optional `--show-candidates <N>`
- Supports base64, base32, hex, and pattern-based detection

### Known Limitations
- **Streaming mode**: Does NOT yet support compression or hashing
  - Line 150-152 in main.rs: Explicit error if compression + streaming
  - No hash integration with streaming mode
  - TODO: Add streaming compression and streaming hashing support

## Code Style & Conventions

### When Making Changes

- **Minimal modifications**: Only change what's necessary to achieve the goal
- **Preserve optimizations**: Fast lookup tables, pre-allocation, chunking patterns
- **Match existing patterns**: Follow established encoder structure in `encoders/` directory
- **No breaking changes**: Keep public API stable (encode, decode, hash, compress functions)
- **Test coverage**: Run `cargo test` (73 tests as of 2024-11) before committing

### Dictionary-Related Code

When working with dictionaries:
- Mode selection is automatic via `Dictionary.mode()` match statements
- ASCII-only dictionaries automatically get fast lookup tables in `Dictionary::new_with_mode_and_range()`
- Always validate dictionary size matches mode requirements (power-of-two for Chunked)
- Use `create_alphabet()` helper pattern in CLI code

### Hashing Code

When working with hashing:
- All hash algorithms use `HashAlgorithm` enum in `src/hashing.rs`
- Use appropriate library: RustCrypto (crypto), crc (checksums), twox-hash (xxHash)
- xxHash3 modules: `twox_hash::xxhash3_64::Hasher`, `twox_hash::xxhash3_128::Hasher`
- Hash output defaults to hex, or encode with `-e <dictionary>` flag
- For xxHash32, cast to u32: `(hasher.finish() as u32).to_be_bytes()`
- For xxHash3-128, use `finish_128()` method, not `finish_ext()`

### Performance-Critical Code

When modifying encoders:
- Pre-allocate with `String::with_capacity()` or `Vec::with_capacity()`
- Process in 64-byte chunks where possible
- Use lookup tables for character-to-index mapping
- Avoid repeated HashMap lookups in hot loops
- Run benchmarks: `cargo bench --bench encoding`

## Common Development Tasks

### Adding a New Dictionary

No code changes needed:
1. Edit `dictionaries.toml`
2. Add entry with `chars`, `mode`, optional `padding` or `start_codepoint`
3. Dictionary is automatically loaded via serde deserialization

### Adding a New Encoding Mode

1. Add variant to `EncodingMode` enum in `src/core/config.rs`
2. Implement encoder/decoder in new file under `src/encoders/`
3. Add to `encoders/mod.rs` exports
4. Add match arms in `encode()` and `decode()` in `src/lib.rs`
5. Update tests in `src/tests.rs`

### Testing & Benchmarking

```bash
cargo test                                     # 38 unit tests + 7 doc tests
cargo bench                                    # Run all benchmarks
cargo bench --bench encoding                   # Specific suite
cargo bench -- --save-baseline name            # Save performance baseline
cargo bench -- --baseline name                 # Compare to baseline
./test_cli.sh                                  # CLI integration tests
```

## CLI Usage Patterns

The CLI supports several modes:
- Encode: `base-d -e <dictionary> [file]`
- Decode: `base-d -d <dictionary> [file]`
- Transcode: `base-d -d base64 -e hex` (no intermediate piping)
- List: `base-d --list` (shows all dictionaries)
- Stream: `base-d --stream -e base64 large_file.bin`
- Matrix effect: `base-d --neo [dictionary]` (defaults to base256_matrix)

If no file specified, reads from stdin.

## Important Implementation Details

### Dictionary Categories

35 built-in dictionaries organized as:
- **RFC Standards**: base16, base32, base32hex, base64, base64url (chunked mode)
- **Blockchain**: base58, base58flickr (math mode)
- **High-Density**: base62, base85, ascii85, z85, base256_matrix, base1024 (math mode)
- **Ancient Scripts**: hieroglyphs, cuneiform, runic (math mode)
- **Game Pieces**: cards, domino, mahjong, chess (math mode)
- **Esoteric**: alchemy, zodiac, weather, music, arrows (math mode)
- **Emoji**: emoji_faces, emoji_animals, base100 (byte_range mode)
- **Other**: dna, binary, hex (various modes)

### Streaming Implementation

Streaming mode uses:
- 4KB buffer size constant (`BUFFER_SIZE`)
- Independent chunk processing for parallel-friendly design
- Separate `StreamingEncoder` and `StreamingDecoder` structs
- Handles large files (multi-GB) with constant memory usage

### Matrix Mode (`--neo`)

Special CLI feature that streams random data encoded with specified dictionary:
- Defaults to base256_matrix (Japanese-style Matrix characters)
- Detects terminal width via `terminal_size` crate
- Continuous random data generation with `rand` crate
- Used for visual effect demonstration

## Dependencies

**Production:**
- num-bigint, num-traits, num-integer: BigUint math for base conversion
- serde, toml: Config file parsing
- clap: CLI argument parsing
- dirs: User config directory discovery
- rand, terminal_size: Matrix mode effects

**Development:**
- criterion: Benchmarking with HTML reports

## Testing Strategy

Tests cover:
- All three encoding modes with multiple dictionaries
- Round-trip encode/decode validation
- Edge cases: empty input, single byte, padding scenarios
- RFC compliance for base64/base32/base16
- Custom dictionary loading from TOML
- Streaming encoder/decoder functionality

All tests must pass before merging changes: `cargo test`

## Future Development

**Check the roadmap and issues:**
- Review `ROADMAP.md` for planned features and optimizations
- Check GitHub Issues at https://github.com/coryzibell/base-d/issues for reported bugs and feature requests
- Consider existing issues before proposing new features

Planned optimizations include:
- SIMD for base64 (potential 4-8x speedup using `std::arch::x86_64` intrinsics)
- Parallel processing with rayon for files >1MB
- Platform-specific optimizations (AVX2/AVX-512 for x86_64, NEON for ARM)

When implementing future features, maintain backward compatibility with existing API.

## Critical Files Reference

- `src/lib.rs` - Public API surface, must remain stable
- `src/main.rs` - CLI entry point and argument parsing (includes streaming mode handling)
- `src/core/dictionary.rs` - Core Dictionary type with lookup optimization
- `src/core/config.rs` - Configuration and EncodingMode enum
- `src/encoders/*.rs` - Algorithm implementations for each mode
- `src/compression.rs` - 6 compression algorithms (pure Rust)
- `src/hashing.rs` - 24 hash algorithms (crypto, CRC, xxHash)
- `src/detection.rs` - Dictionary auto-detection logic
- `dictionaries.toml` - Built-in dictionary definitions
- `benches/encoding.rs` - Performance benchmarks

## Documentation

- `README.md` - User-facing overview and quick start
- `docs/DICTIONARIES.md` - Complete dictionary reference
- `docs/ENCODING_MODES.md` - Detailed algorithm explanations
- `docs/CUSTOM_DICTIONARIES.md` - User dictionary creation guide
- `docs/STREAMING.md` - Large file processing documentation
- `docs/PERFORMANCE.md` - Optimization implementation details
- `docs/COMPRESSION.md` - Compression algorithms and usage
- `docs/HASHING.md` - Hash algorithms, performance comparison, use cases
- `docs/DETECTION.md` - Auto-detection feature documentation
- `OPTIMIZATION_SUMMARY.md` - Recent performance optimization work
- `COMPRESSION_FEATURE.md` - Compression implementation details

When updating functionality, keep relevant documentation in sync.
