# base-d

![base-d demo](assets/impressive-based.gif)

[![Crates.io](https://img.shields.io/crates/v/base-d.svg)](https://crates.io/crates/base-d)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

A universal, multi-dictionary encoding library and CLI tool for Rust. Encode binary data using numerous dictionaries including RFC standards, ancient scripts, emoji, playing cards, Matrix-style Japanese, and more.

## Overview

base-d is a flexible encoding framework that goes far beyond traditional base64. It supports:

- **Numerous built-in dictionaries** - From RFC 4648 standards to hieroglyphics, emoji, Matrix-style base256, and a 1024-character CJK dictionary
- **3 encoding modes** - Mathematical, chunked (RFC-compliant), and byte-range
- **Auto-detection** - Automatically identify which dictionary was used to encode data
- **Compression support** - Built-in gzip, zstd, brotli, lz4, snappy, and lzma compression with configurable levels
- **Hashing support** - 26 hash algorithms: cryptographic (SHA-256, BLAKE3, Ascon, etc.), CRC checksums, and xxHash including xxHash3 (pure Rust, no OpenSSL)
- **Custom dictionaries** - Define your own via TOML configuration
- **Streaming support** - Memory-efficient processing for large files
- **Library + CLI** - Use programmatically or from the command line
- **High performance** - Optimized with fast lookup tables and efficient memory allocation
- **Special encodings** - Matrix-style base256 that works like hex (1:1 byte mapping)

## Key Features

### Multiple Encoding Modes

1. **Mathematical Base Conversion** - Treats data as a large number, works with any dictionary size
2. **Chunked Mode** - RFC 4648 compatible (base64, base32, base16)
3. **Byte Range Mode** - Direct 1:1 byte-to-emoji mapping (base100)

### Performance

- **SIMD-Accelerated** - Runtime AVX2/SSSE3 (x86_64) and NEON (ARM) detection
- **Specialized SIMD** - Hardcoded lookup tables for RFC dictionaries (base64, base32, base16)
- **LUT SIMD** - Runtime lookup tables for arbitrary dictionaries
- **~500 MiB/s** base64 encode, **~7.4 GiB/s** base64 decode with specialized SIMD
- **Streaming Mode** - Process multi-GB files with constant 4KB memory usage

### Extensive Dictionary Collection

- **Standards**: base64, base32, base16, base58 (Bitcoin), base85 (Git)
- **Ancient Scripts**: Egyptian hieroglyphics, Sumerian cuneiform, Elder Futhark runes
- **Game Pieces**: Playing cards, mahjong tiles, domino tiles, chess pieces
- **Esoteric**: Alchemical symbols, zodiac signs, weather symbols, musical notation
- **Emoji**: Face emoji, animal emoji, base100 (256 emoji range)
- **Custom**: Define your own dictionaries in TOML

### Advanced Capabilities

- **Streaming Mode** - Process multi-GB files with constant 4KB memory usage
- **Dictionary Detection** - Automatically identify encoding format without prior knowledge
- **Compression Pipeline** - Compress before encoding with gzip, zstd, brotli, or lz4
- **User Configuration** - Load custom dictionaries from `~/.config/base-d/dictionaries.toml`
- **Project-Local Config** - Override dictionaries per-project with `./dictionaries.toml`
- **Three Independent Algorithms** - Choose the right mode for your use case

## Quick Start

```bash
# Install (once published)
cargo install base-d

# Or build from source
git clone https://github.com/yourusername/base-d
cd base-d
cargo build --release

# List all available dictionaries
base-d config list

# Encode with playing cards (default)
echo "Secret message" | base-d encode

# RFC 4648 base32
echo "Data" | base-d encode base32

# Bitcoin base58
echo "Address" | base-d encode base58

# Egyptian hieroglyphics
echo "Ancient" | base-d encode hieroglyphs

# Emoji faces
echo "Happy" | base-d encode emoji_faces

# Matrix-style base256
echo "Wake up, Neo" | base-d encode base256_matrix

# Enter the Matrix (live streaming random Matrix code)
base-d neo

# Auto-detect dictionary and decode
echo "SGVsbG8sIFdvcmxkIQ==" | base-d detect

# Show top candidates with confidence scores
base-d detect --show-candidates 5 input.txt

# Transcode between dictionaries (decode from one, encode to another)
echo "SGVsbG8=" | base-d decode base64 --encode hex
echo "48656c6c6f" | base-d decode hex --encode emoji_faces

# Compress and encode (supported: gzip, zstd, brotli, lz4, snappy, lzma)
echo "Data to compress" | base-d encode base64 --compress gzip
echo "Large file" | base-d encode base85 --compress zstd --level 9
echo "Fast compression" | base-d encode base64 --compress snappy

# Compress with default encoding (base64)
echo "Quick compress" | base-d encode --compress gzip

# Decompress and decode
echo "H4sIAAAAAAAA/..." | base-d decode base64 --decompress gzip

# Output raw compressed binary
echo "Data" | base-d encode --compress zstd --raw > output.zst

# Process files
base-d encode base64 input.txt > encoded.txt
base-d decode base64 encoded.txt > output.txt

# Compress large files efficiently
base-d encode base64 --compress brotli --level 11 large_file.bin > compressed.txt

# Hash files (supported: md5, sha256, sha512, blake3, ascon, k12, crc32, xxhash64, xxhash3, and more)
echo "hello world" | base-d hash sha256
echo "hello world" | base-d hash blake3 --encode base64
echo "hello world" | base-d hash ascon
echo "hello world" | base-d hash k12
echo "hello world" | base-d hash crc32
echo "hello world" | base-d hash xxhash3
base-d hash sha256 document.pdf

# Hash with custom seed
echo "hello world" | base-d hash xxhash64 --seed 42

# Hash with secret (XXH3 only)
cat secret.bin | base-d hash xxhash3 --secret-stdin data.bin
```

## Installation

```bash
cargo install base-d
```

## Schema Encoding

LLM-to-LLM wire protocol for structured data. Binary-packed, display-safe, parser-inert.

```bash
# Encode JSON
echo '{"users":[{"id":1,"name":"alice"}]}' | base-d schema
# Output: 𓍹╣◟╥◕◝▰◣◥▟╺▖◘▰◝▤◀╧𓍺

# Decode
echo '𓍹╣◟╥◕◝▰◣◥▟╺▖◘▰◝▤◀╧𓍺' | base-d schema -d

# With compression
base-d schema -c brotli input.json

# Pretty output
base-d schema -d --pretty encoded.txt
```

See [SCHEMA.md](SCHEMA.md) for format specification.

## Usage

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
base-d = "0.1"
```

#### Basic Encoding/Decoding

```rust
use base_d::{DictionariesConfig, Dictionary, encode, decode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load built-in dictionaries
    let config = DictionariesConfig::load_default()?;
    let dict_config = config.get_dictionary("base64").unwrap();

    // Create dictionary from config
    let chars: Vec<char> = dict_config.chars.chars().collect();
    let padding = dict_config.padding.as_ref().and_then(|s| s.chars().next());
    let dictionary = Dictionary::new_with_mode(
        chars,
        dict_config.mode.clone(),
        padding
    )?;
    
    // Encode
    let data = b"Hello, World!";
    let encoded = encode(data, &dictionary);
    println!("Encoded: {}", encoded); // SGVsbG8sIFdvcmxkIQ==
    
    // Decode
    let decoded = decode(&encoded, &dictionary)?;
    assert_eq!(data, &decoded[..]);
    
    Ok(())
}
```

#### Streaming for Large Files

```rust
use base_d::{DictionariesConfig, StreamingEncoder, StreamingDecoder};
use std::fs::File;

fn stream_encode() -> Result<(), Box<dyn std::error::Error>> {
    let config = DictionariesConfig::load_default()?;
    let dict_config = config.get_dictionary("base64").unwrap();

    // ... create dictionary (same as above)
    
    let mut input = File::open("large_file.bin")?;
    let mut output = File::create("encoded.txt")?;
    
    let mut encoder = StreamingEncoder::new(&dictionary, output);
    encoder.encode(&mut input)?;
    
    Ok(())
}
```

#### Custom Dictionaries

```rust
use base_d::{Dictionary, EncodingMode, encode};

fn custom_dictionary() -> Result<(), Box<dyn std::error::Error>> {
    // Define a custom dictionary
    let chars: Vec<char> = "😀😁😂🤣😃😄😅😆😉😊😋😎😍😘🥰😗".chars().collect();
    let dictionary = Dictionary::new_with_mode(
        chars,
        EncodingMode::BaseConversion,
        None
    )?;
    
    let encoded = encode(b"Hi", &dictionary);
    println!("{}", encoded); // 😍😁
    
    Ok(())
}
```

#### Loading User Configurations

```rust
use base_d::DictionariesConfig;

// Load with user overrides from:
// 1. Built-in dictionaries
// 2. ~/.config/base-d/dictionaries.toml  
// 3. ./dictionaries.toml
let config = DictionariesConfig::load_with_overrides()?;

// Or load from specific file
let config = DictionariesConfig::load_from_file("custom.toml".as_ref())?;
```

### As a CLI Tool

Encode and decode data using any dictionary defined in `dictionaries.toml`:

```bash
# List available dictionaries
base-d config list

# Encode from stdin (default dictionary is "cards")
echo "Hello, World!" | base-d encode

# Encode a file
base-d encode input.txt

# Encode with specific dictionary
echo "Data" | base-d encode dna

# Decode from specific dictionary
echo "SGVsbG8gV29ybGQNCg==" | base-d decode base64

# Decode playing cards
echo "🃎🃅🃝🃉🂡🂣🂸🃉🃉🃇🃉🃓🂵🂣🂨🂻🃆🃍" | base-d decode cards

# Transcode between dictionaries (no intermediate piping needed!)
echo "SGVsbG8=" | base-d decode base64 --encode hex
# Output: 48656c6c6f

# Convert between any two dictionaries
echo "ACGTACGT" | base-d decode dna --encode emoji_faces
echo "🃁🃂🃃🃄" | base-d decode cards --encode base64

# Stream mode for large files (memory efficient)
base-d encode base64 --stream large_file.bin > encoded.txt
base-d decode base64 --stream encoded.txt > decoded.bin
```

### Custom Dictionaries

Add your own dictionaries to `dictionaries.toml`:

```toml
[dictionaries]
# Your custom 16-character dictionary
hex_emoji = "😀😁😂🤣😃😄😅😆😉😊😋😎😍😘🥰😗"

# Chess pieces (12 characters)
chess = "♔♕♖♗♘♙♚♛♜♝♞♟"
```

Or create custom dictionaries in `~/.config/base-d/dictionaries.toml` to use across all projects. See [Custom Dictionaries Guide](docs/CUSTOM_DICTIONARIES.md) for details.

## Built-in Dictionaries

base-d includes 35 pre-configured dictionaries organized into several categories:

- **RFC 4648 Standards**: base16, base32, base32hex, base64, base64url
- **Bitcoin & Blockchain**: base58, base58flickr
- **High-Density Encodings**: base62, base85, ascii85, z85, base256_matrix (Matrix-style), base1024
- **Human-Oriented**: base32_crockford, base32_zbase
- **Ancient Scripts**: hieroglyphs, cuneiform, runic
- **Game Pieces**: cards, domino, mahjong, chess
- **Esoteric Symbols**: alchemy, zodiac, weather, music, arrows
- **Emoji**: emoji_faces, emoji_animals, base100
- **Other**: dna, binary, hex, base64_math, hex_math

Run `base-d --list` to see all available dictionaries with their encoding modes.

For a complete reference with examples and use cases, see [DICTIONARIES.md](docs/DICTIONARIES.md).

## How It Works

base-d supports three encoding algorithms:

1. **Mathematical Base Conversion** (default) - Treats binary data as a single large number and converts it to the target base. Works with any dictionary size.

2. **Bit-Chunking** - Groups bits into fixed-size chunks for RFC 4648 compatibility (base64, base32, base16).

3. **Byte Range** - Direct 1:1 byte-to-character mapping using a Unicode range (like base100). Each byte maps to a specific emoji with zero encoding overhead.

For a detailed explanation of all modes with examples, see [ENCODING_MODES.md](docs/ENCODING_MODES.md).

## License

MIT OR Apache-2.0

## Documentation

### Core Concepts
- [Dictionary Reference](docs/DICTIONARIES.md) - Complete guide to all built-in dictionaries
- [Custom Dictionaries](docs/CUSTOM_DICTIONARIES.md) - Create and load your own dictionaries
- [Encoding Modes](docs/ENCODING_MODES.md) - Mathematical vs chunked vs byte range encoding
- [Base1024](docs/BASE1024.md) - High-density CJK encoding

### Features
- [Hashing](docs/HASHING.md) - 24 hash algorithms (SHA, BLAKE, CRC, xxHash)
- [Compression](docs/COMPRESSION.md) - gzip, zstd, brotli, lz4, snappy, lzma support
- [Detection](docs/DETECTION.md) - Auto-detect encoding format
- [Streaming](docs/STREAMING.md) - Memory-efficient processing for large files

### Performance
- [SIMD Optimizations](docs/SIMD.md) - AVX2/SSSE3/NEON acceleration
- [Benchmarking](docs/BENCHMARKING.md) - Running and interpreting benchmarks
- [Performance Guide](docs/PERFORMANCE.md) - Benchmarks and optimization tips

### Matrix Mode
- [Matrix Mode](docs/MATRIX.md) - Live Matrix-style visualization
- [Neo Mode](docs/NEO.md) - `--neo` flag deep dive

### Reference
- [API Reference](docs/API.md) - Library API documentation
- [Hexadecimal Explained](docs/HEX_EXPLANATION.md) - Why hex is special
- [Roadmap](docs/ROADMAP.md) - Planned features and development phases
- [CI/CD Setup](docs/CI_CD.md) - GitHub Actions workflow documentation

## Contributing

Contributions are welcome! Please see [ROADMAP.md](docs/ROADMAP.md) for planned features.
# sync test
