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
- **Hashing support** - 22 hash algorithms: cryptographic (SHA-256, BLAKE3, etc.), CRC checksums, and xxHash (pure Rust, no OpenSSL)
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
base-d --list

# Encode with playing cards (default)
echo "Secret message" | base-d

# RFC 4648 base32
echo "Data" | base-d -e base32

# Bitcoin base58
echo "Address" | base-d -e base58

# Egyptian hieroglyphics
echo "Ancient" | base-d -e hieroglyphs

# Emoji faces
echo "Happy" | base-d -e emoji_faces

# Matrix-style base256
echo "Wake up, Neo" | base-d -e base256_matrix

# Enter the Matrix (live streaming random Matrix code)
base-d --neo

# Auto-detect dictionary and decode
echo "SGVsbG8sIFdvcmxkIQ==" | base-d --detect

# Show top candidates with confidence scores
base-d --detect --show-candidates 5 input.txt

# Transcode between dictionaries (decode from one, encode to another)
echo "SGVsbG8=" | base-d -d base64 -e hex
echo "48656c6c6f" | base-d -d hex -e emoji_faces

# Compress and encode (supported: gzip, zstd, brotli, lz4, snappy, lzma)
echo "Data to compress" | base-d --compress gzip -e base64
echo "Large file" | base-d --compress zstd --level 9 -e base85
echo "Fast compression" | base-d --compress snappy -e base64

# Compress with default encoding (base64)
echo "Quick compress" | base-d --compress gzip

# Decompress and decode
echo "H4sIAAAAAAAA/..." | base-d -d base64 --decompress gzip

# Output raw compressed binary
echo "Data" | base-d --compress zstd --raw > output.zst

# Process files
base-d -e base64 input.txt > encoded.txt
base-d -d base64 encoded.txt > output.txt

# Compress large files efficiently
base-d --compress brotli --level 11 -e base64 large_file.bin > compressed.txt

# Hash files (supported: md5, sha256, sha512, blake3, crc32, xxhash64, and more)
echo "hello world" | base-d --hash sha256
echo "hello world" | base-d --hash blake3 -e base64
echo "hello world" | base-d --hash crc32
echo "hello world" | base-d --hash xxhash64
base-d --hash sha256 document.pdf
```

## Installation

```bash
cargo install base-d
```

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
    let alphabet_config = config.get_dictionary("base64").unwrap();
    
    // Create dictionary from config
    let chars: Vec<char> = alphabet_config.chars.chars().collect();
    let padding = alphabet_config.padding.as_ref().and_then(|s| s.chars().next());
    let dictionary = Dictionary::new_with_mode(
        chars, 
        alphabet_config.mode.clone(), 
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
    let alphabet_config = config.get_dictionary("base64").unwrap();
    
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

fn custom_alphabet() -> Result<(), Box<dyn std::error::Error>> {
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
base-d --list

# Encode from stdin (default dictionary is "cards")
echo "Hello, World!" | base-d

# Encode a file
base-d input.txt

# Encode with specific dictionary
echo "Data" | base-d -e dna

# Decode from specific dictionary
echo "SGVsbG8gV29ybGQNCg==" | base-d -d base64

# Decode playing cards
echo "🃎🃅🃝🃉🂡🂣🂸🃉🃉🃇🃉🃓🂵🂣🂨🂻🃆🃍" | base-d -d cards

# Transcode between dictionaries (no intermediate piping needed!)
echo "SGVsbG8=" | base-d -d base64 -e hex
# Output: 48656c6c6f

# Convert between any two dictionaries
echo "ACGTACGT" | base-d -d dna -e emoji_faces
echo "🃁🃂🃃🃄" | base-d -d cards -e base64

# Stream mode for large files (memory efficient)
base-d --stream -e base64 large_file.bin > encoded.txt
base-d --stream -d base64 encoded.txt > decoded.bin
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

- [Dictionary Reference](docs/DICTIONARIES.md) - Complete guide to all numerous built-in dictionaries
- [Custom Dictionaries](docs/CUSTOM_DICTIONARIES.md) - Create and load your own dictionaries
- [Encoding Modes](docs/ENCODING_MODES.md) - Detailed explanation of mathematical vs chunked vs byte range encoding
- [Streaming](docs/STREAMING.md) - Memory-efficient processing for large files
- [Hexadecimal Explained](docs/HEX_EXPLANATION.md) - Special case where both modes produce identical output
- [Roadmap](docs/ROADMAP.md) - Planned features and development phases
- [CI/CD Setup](docs/CI_CD.md) - GitHub Actions workflow documentation

## Contributing

Contributions are welcome! Please see [ROADMAP.md](docs/ROADMAP.md) for planned features.
