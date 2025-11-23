# base-d

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

base-d includes **32 pre-configured alphabets**:

### RFC 4648 Standards (Chunked Mode)
- **base16** - Uppercase hexadecimal
- **base32** - RFC 4648 base32
- **base32hex** - RFC 4648 base32 extended hex
- **base64** - Standard base64
- **base64url** - URL-safe base64

### Bitcoin & Blockchain (Mathematical Mode)
- **base58** - Bitcoin addresses (no 0, O, I, l)
- **base58flickr** - Flickr variant

### High-Density Encodings (Mathematical Mode)
- **base62** - Alphanumeric (URL shorteners)
- **base85** - Git pack format
- **ascii85** - Adobe PDF encoding
- **z85** - ZeroMQ encoding

### Human-Oriented (Mathematical Mode)
- **base32_crockford** - Douglas Crockford's base32 (no ambiguous chars)
- **base32_zbase** - z-base-32 (designed for human use)

### Ancient Scripts (Mathematical Mode)
- **hieroglyphs** - Egyptian hieroglyphics (100 chars) 𓀀𓀁𓀂
- **cuneiform** - Sumerian cuneiform (100 chars) 𒀀𒀁𒀂
- **runic** - Elder Futhark & variants (81 chars) ᚠᚡᚢ

### Game Pieces (Mathematical Mode)
- **cards** - 52 Unicode playing cards 🂡🂾🃁🃞
- **domino** - Domino tiles (100 chars) 🀰🀱🀲
- **mahjong** - Mahjong tiles (44 chars) 🀀🀁🀂
- **chess** - Chess pieces (12 chars) ♔♕♖♗♘♙

### Esoteric Symbols (Mathematical Mode)
- **alchemy** - Alchemical symbols (116 chars) 🜀🜁🜂
- **zodiac** - Zodiac signs (12 chars) ♈♉♊
- **weather** - Weather & misc symbols (72 chars) ☀☁☂
- **music** - Musical notation (100 chars) 𝄀𝄁𝄂
- **arrows** - Arrow symbols (112 chars) ←↑→↓

### Emoji (Mathematical Mode)
- **emoji_faces** - Emoji faces (80 chars) 😀😁😂
- **emoji_animals** - Animal emoji (64 chars) 🐀🐁🐂

### Fun & Creative (Mathematical Mode)
- **dna** - DNA nucleotides (ACGT)
- **binary** - Binary (01)
- **hex** - Lowercase hexadecimal

### Mathematical Variants
- **base64_math** - Base64 with mathematical encoding
- **hex_math** - Hex with mathematical encoding

Run `base-d --list` to see all available alphabets with their encoding modes.

## How It Works

base-d supports two encoding algorithms:

### 1. Mathematical Base Conversion (default)

Treats binary data as a single large number and converts it to the target base:

- `"Hello, World!"` (13 bytes) → `🃎🃅🃝🃉🂡🂣🂸🃉🃉🃇🃉🃓🂵🂣🂨🂻🃆🃍` (18 cards)
- Each character represents a digit in base-N
- Leading zeros are preserved
- No padding needed
- Works with ANY alphabet size

**Best for:** Playing cards, DNA, emoji, and custom alphabets

### 2. Bit-Chunking (for RFC standards)

Groups bits into fixed-size chunks, like standard base64:

- Processes data in fixed bit-width groups
- Compatible with RFC 4648 (standard base64)
- Requires power-of-2 alphabet sizes (2, 4, 8, 16, 32, 64, etc.)
- Supports padding characters

**Best for:** Standard base64, base32, base16, and other RFC-compliant encodings

### Alphabet Configuration

Specify the encoding mode in `alphabets.toml`:

```toml
[alphabets.base64]
chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
mode = "chunked"        # RFC 4648 compatible
padding = "="

[alphabets.cards]
chars = "🂡🂢🂣..."
mode = "base_conversion"  # Mathematical (default)
```

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Please see [ROADMAP.md](ROADMAP.md) for planned features.
