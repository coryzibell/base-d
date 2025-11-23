# base-d

![base-d demo](assets/impressive-based.gif)

[![Crates.io](https://img.shields.io/crates/v/base-d.svg)](https://crates.io/crates/base-d)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

A universal, multi-alphabet encoding library and CLI tool for Rust. Encode binary data to 33+ alphabets including RFC standards, ancient scripts, emoji, playing cards, and more.

## Overview

base-d is a flexible encoding framework that goes far beyond traditional base64. It supports:

- **33 built-in alphabets** - From RFC 4648 standards to hieroglyphics and emoji
- **3 encoding modes** - Mathematical, chunked (RFC-compliant), and byte-range
- **Custom alphabets** - Define your own via TOML configuration
- **Streaming support** - Memory-efficient processing for large files
- **Library + CLI** - Use programmatically or from the command line

## Key Features

### Multiple Encoding Modes

1. **Mathematical Base Conversion** - Treats data as a large number, works with any alphabet size
2. **Chunked Mode** - RFC 4648 compatible (base64, base32, base16)
3. **Byte Range Mode** - Direct 1:1 byte-to-emoji mapping (base100)

### Extensive Alphabet Collection

- **Standards**: base64, base32, base16, base58 (Bitcoin), base85 (Git)
- **Ancient Scripts**: Egyptian hieroglyphics, Sumerian cuneiform, Elder Futhark runes
- **Game Pieces**: Playing cards, mahjong tiles, domino tiles, chess pieces
- **Esoteric**: Alchemical symbols, zodiac signs, weather symbols, musical notation
- **Emoji**: Face emoji, animal emoji, base100 (256 emoji range)
- **Custom**: Define your own alphabets in TOML

### Advanced Capabilities

- **Streaming Mode** - Process multi-GB files with constant 4KB memory usage
- **User Configuration** - Load custom alphabets from `~/.config/base-d/alphabets.toml`
- **Project-Local Config** - Override alphabets per-project with `./alphabets.toml`
- **Three Independent Algorithms** - Choose the right mode for your use case

## Quick Start

```bash
# Install (once published)
cargo install base-d

# Or build from source
git clone https://github.com/yourusername/base-d
cd base-d
cargo build --release

# List all 33 available alphabets
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

# Transcode between alphabets (decode from one, encode to another)
echo "SGVsbG8=" | base-d --from base64 -a hex
echo "48656c6c6f" | base-d --from hex -a emoji_faces

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

Add to your `Cargo.toml`:

```toml
[dependencies]
base-d = "0.1"
```

#### Basic Encoding/Decoding

```rust
use base_d::{AlphabetsConfig, Alphabet, encode, decode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load built-in alphabets
    let config = AlphabetsConfig::load_default()?;
    let alphabet_config = config.get_alphabet("base64").unwrap();
    
    // Create alphabet from config
    let chars: Vec<char> = alphabet_config.chars.chars().collect();
    let padding = alphabet_config.padding.as_ref().and_then(|s| s.chars().next());
    let alphabet = Alphabet::new_with_mode(
        chars, 
        alphabet_config.mode.clone(), 
        padding
    )?;
    
    // Encode
    let data = b"Hello, World!";
    let encoded = encode(data, &alphabet);
    println!("Encoded: {}", encoded); // SGVsbG8sIFdvcmxkIQ==
    
    // Decode
    let decoded = decode(&encoded, &alphabet)?;
    assert_eq!(data, &decoded[..]);
    
    Ok(())
}
```

#### Streaming for Large Files

```rust
use base_d::{AlphabetsConfig, StreamingEncoder, StreamingDecoder};
use std::fs::File;

fn stream_encode() -> Result<(), Box<dyn std::error::Error>> {
    let config = AlphabetsConfig::load_default()?;
    let alphabet_config = config.get_alphabet("base64").unwrap();
    
    // ... create alphabet (same as above)
    
    let mut input = File::open("large_file.bin")?;
    let mut output = File::create("encoded.txt")?;
    
    let mut encoder = StreamingEncoder::new(&alphabet, output);
    encoder.encode(&mut input)?;
    
    Ok(())
}
```

#### Custom Alphabets

```rust
use base_d::{Alphabet, EncodingMode, encode};

fn custom_alphabet() -> Result<(), Box<dyn std::error::Error>> {
    // Define a custom alphabet
    let chars: Vec<char> = "😀😁😂🤣😃😄😅😆😉😊😋😎😍😘🥰😗".chars().collect();
    let alphabet = Alphabet::new_with_mode(
        chars,
        EncodingMode::BaseConversion,
        None
    )?;
    
    let encoded = encode(b"Hi", &alphabet);
    println!("{}", encoded); // 😍😁
    
    Ok(())
}
```

#### Loading User Configurations

```rust
use base_d::AlphabetsConfig;

// Load with user overrides from:
// 1. Built-in alphabets
// 2. ~/.config/base-d/alphabets.toml  
// 3. ./alphabets.toml
let config = AlphabetsConfig::load_with_overrides()?;

// Or load from specific file
let config = AlphabetsConfig::load_from_file("custom.toml".as_ref())?;
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

# Transcode between alphabets (no intermediate piping needed!)
echo "SGVsbG8=" | base-d --from base64 -a hex
# Output: 48656c6c6f

# Convert between any two alphabets
echo "ACGTACGT" | base-d --from dna -a emoji_faces
echo "🃁🃂🃃🃄" | base-d --from cards -a base64

# Stream mode for large files (memory efficient)
base-d --stream -a base64 large_file.bin > encoded.txt
base-d --stream -a base64 -d encoded.txt > decoded.bin
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

Or create custom alphabets in `~/.config/base-d/alphabets.toml` to use across all projects. See [Custom Alphabets Guide](docs/CUSTOM_ALPHABETS.md) for details.

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

- [Alphabet Reference](docs/ALPHABETS.md) - Complete guide to all 33 built-in alphabets
- [Custom Alphabets](docs/CUSTOM_ALPHABETS.md) - Create and load your own alphabets
- [Encoding Modes](docs/ENCODING_MODES.md) - Detailed explanation of mathematical vs chunked vs byte range encoding
- [Streaming](docs/STREAMING.md) - Memory-efficient processing for large files
- [Hexadecimal Explained](docs/HEX_EXPLANATION.md) - Special case where both modes produce identical output
- [Roadmap](docs/ROADMAP.md) - Planned features and development phases
- [CI/CD Setup](docs/CI_CD.md) - GitHub Actions workflow documentation

## Contributing

Contributions are welcome! Please see [ROADMAP.md](docs/ROADMAP.md) for planned features.
