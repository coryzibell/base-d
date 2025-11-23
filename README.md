# base-d

![base-d demo](assets/impressive-based.gif)

[![Crates.io](https://img.shields.io/crates/v/base-d.svg)](https://crates.io/crates/base-d)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

A Rust library and CLI tool for encoding binary data using esoteric, curated alphabets.

## Overview

Similar to how base58 encodes binary data to a carefully selected set of characters, base-d provides encoding and decoding functionality for various custom alphabets. Define alphabets in a simple TOML configuration file, or use the built-in alphabets.

## Features

- **TOML-based Alphabet Configuration**: Define custom alphabets in `alphabets.toml`
- **Multiple Alphabet Support**: Built-in alphabets and easy custom alphabet creation
- **Playing Card Alphabet**: 52-character encoding using Unicode playing card symbols
- **Library and Binary**: Use as a Rust crate or standalone CLI tool
- **Efficient Encoding**: Fast binary-to-alphabet conversion using arbitrary-precision arithmetic

## Quick Start

```bash
# Install (once published)
cargo install base-d

# Or build from source
git clone https://github.com/yourusername/base-d
cd base-d
cargo build --release

# List all 32 available alphabets
base-d --list

# Encode with playing cards (default)
echo "Secret message" | base-d

# RFC 4648 base32
echo "Data" | base-d -a base32

# Bitcoin base58
echo "Address" | base-d -a base58

# Egyptian hieroglyphics
echo "Ancient" | base-d -a hieroglyphs

# Emoji faces
echo "Happy" | base-d -a emoji_faces

# Process files
base-d input.txt > encoded.txt
base-d -d encoded.txt > output.txt
```

## Installation

```bash
cargo install base-d
```

## Usage

### As a Library

```rust
use base_d::{AlphabetsConfig, Alphabet, encode, decode};

fn main() {
    // Load alphabets from configuration
    let config = AlphabetsConfig::load_default().unwrap();
    let cards_str = config.get_alphabet("cards").unwrap();
    let alphabet = Alphabet::from_str(cards_str).unwrap();
    
    // Encode data
    let data = b"Hello, World!";
    let encoded = encode(data, &alphabet);
    println!("Encoded: {}", encoded);
    
    // Decode data
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(data, &decoded[..]);
}
```

### As a CLI Tool

Encode and decode data using any alphabet defined in `alphabets.toml`:

```bash
# List available alphabets
base-d --list

# Encode from stdin (default alphabet is "cards")
echo "Hello, World!" | base-d

# Encode a file
base-d input.txt

# Encode with specific alphabet
echo "Data" | base-d -a dna

# Decode
echo "🃎🃅🃝🃉🂡🂣🂸🃉🃉🃇🃉🃓🂵🂣🂨🂻🃆🃍" | base-d -d

# Round-trip encoding
echo "Secret" | base-d | base-d -d
```

### Custom Alphabets

Add your own alphabets to `alphabets.toml`:

```toml
[alphabets]
# Your custom 16-character alphabet
hex_emoji = "😀😁😂🤣😃😄😅😆😉😊😋😎😍😘🥰😗"

# Chess pieces (12 characters)
chess = "♔♕♖♗♘♙♚♛♜♝♞♟"
```

## Built-in Alphabets

base-d includes 33 pre-configured alphabets organized into several categories:

- **RFC 4648 Standards**: base16, base32, base32hex, base64, base64url
- **Bitcoin & Blockchain**: base58, base58flickr
- **High-Density Encodings**: base62, base85, ascii85, z85
- **Human-Oriented**: base32_crockford, base32_zbase
- **Ancient Scripts**: hieroglyphs, cuneiform, runic
- **Game Pieces**: cards, domino, mahjong, chess
- **Esoteric Symbols**: alchemy, zodiac, weather, music, arrows
- **Emoji**: emoji_faces, emoji_animals, base100
- **Other**: dna, binary, hex, base64_math, hex_math

Run `base-d --list` to see all available alphabets with their encoding modes.

For a complete reference with examples and use cases, see [ALPHABETS.md](docs/ALPHABETS.md).

## How It Works

base-d supports three encoding algorithms:

1. **Mathematical Base Conversion** (default) - Treats binary data as a single large number and converts it to the target base. Works with any alphabet size.

2. **Bit-Chunking** - Groups bits into fixed-size chunks for RFC 4648 compatibility (base64, base32, base16).

3. **Byte Range** - Direct 1:1 byte-to-character mapping using a Unicode range (like base100). Each byte maps to a specific emoji with zero encoding overhead.

For a detailed explanation of all modes with examples, see [ENCODING_MODES.md](docs/ENCODING_MODES.md).

## License

MIT OR Apache-2.0

## Documentation

- [Alphabet Reference](docs/ALPHABETS.md) - Complete guide to all 32 built-in alphabets
- [Encoding Modes](docs/ENCODING_MODES.md) - Detailed explanation of mathematical vs chunked encoding
- [Hexadecimal Explained](docs/HEX_EXPLANATION.md) - Special case where both modes produce identical output
- [Roadmap](docs/ROADMAP.md) - Planned features and development phases
- [CI/CD Setup](docs/CI_CD.md) - GitHub Actions workflow documentation

## Contributing

Contributions are welcome! Please see [ROADMAP.md](docs/ROADMAP.md) for planned features.
