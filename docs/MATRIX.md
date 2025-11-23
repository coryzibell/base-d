# Matrix Base256 Encoding - Like Hexadecimal, But Cooler

## Overview

**base256_matrix** is a special 256-character alphabet that uses Japanese characters and Unicode shapes to create a "Matrix"-style encoding. It has the unique property of working identically in both mathematical and chunked encoding modes - just like hexadecimal.

## The Magic Property

```
Base256 = 2^8 (8 bits per character = 1 byte)
8 bits % 8 = 0 (perfect division)
Result: BOTH modes produce IDENTICAL output
```

This is the same mathematical property that makes hexadecimal work in both modes (see `docs/HEX_EXPLANATION.md`).

## Character Composition

The alphabet uses 256 carefully selected Unicode characters:

| Block | Count | Range | Examples |
|-------|-------|-------|----------|
| **Hiragana** | 83 | U+3041-U+3093 | ぁあぃいぅうぇえぉおか |
| **Katakana** | 96 | U+30A0-U+30FF | ゠ァアィイゥウェエォオカ |
| **Box Drawing** | 32 | U+2500-U+251F | ─━│┃┄┅┆┇┈┉┊┋ |
| **Geometric Shapes** | 16 | U+25A0-U+25AF | ■□▢▣▤▥▦▧▨▩▪▫ |
| **Block Elements** | 32 | U+2580-U+259F | ▀▁▂▃▄▅▆▇█▉▊▋ |

Total: **256 characters** (trimmed to exactly 256 from 259 available)

## Visual Style

The encoding looks like the falling code from The Matrix:

```
Original: Wake up, Neo...
Encoded:  イギジゲちヂソねちわゲゼははは

Original: Follow the white rabbit
Encoded:  ょゼススゼツちチサゲちツサザチゲちタギククザチ

Original: There is no spoon
Encoded:  ァサゲタゲちザダちセゼちダソゼゼセ
```

## Properties

### 1:1 Byte Mapping
```
Input bytes:  5
Output chars: 5
No expansion
```

Unlike base64 (which expands ~33%) or even hexadecimal (which doubles), base256_matrix maintains a **perfect 1:1 ratio**. Each byte maps to exactly one character.

### Perfect Efficiency
```
Bits per character:
  - Hex:     4 bits (2 chars per byte)
  - Base64:  6 bits (4 chars per 3 bytes)
  - Base256: 8 bits (1 char per byte) ⭐
```

### Mode Independence

Both encoding modes produce identical output:

```bash
echo "Matrix" | base-d -e base256_matrix
# Chunked mode:     ゎギチタザヅ
# Mathematical mode: ゎギチタザヅ
# IDENTICAL
```

## Usage

### CLI

```bash
# Encode
echo "The Matrix has you" | base-d -e base256_matrix
# Output: ァサゲちゎギチタザヅちサギダちテゼヂぎか

# Decode
echo "ァサゲちゎギチタザヅちサギダちテゼヂぎか" | base-d -d base256_matrix
# Output: The Matrix has you

# List alphabets
base-d --list | grep matrix
```

### Library

```rust
use base_d::{AlphabetsConfig, Alphabet, encode, decode};

let config = AlphabetsConfig::load_default()?;
let matrix_config = config.get_alphabet("base256_matrix")?;

let chars: Vec<char> = matrix_config.chars.chars().collect();
let alphabet = Alphabet::new_with_mode(
    chars,
    matrix_config.mode.clone(),
    None
)?;

let data = b"Free your mind";
let encoded = encode(data, &alphabet);
// ょタゲゲちテゼヂタちズザセケ

let decoded = decode(&encoded, &alphabet)?;
assert_eq!(decoded, data);
```

## Comparison with Other Bases

| Encoding | Bits/Char | Expansion | Speed | Visual Style |
|----------|-----------|-----------|-------|--------------|
| Hex | 4 | 2x | Fast | Boring 0-9A-F |
| Base64 | 6 | 1.33x | Very Fast | Standard ASCII |
| Base100 | 8 | 1x | Very Fast | Emoji |
| **Base256 Matrix** | 8 | **1x** | Fast | **Matrix-style** 🟢 |
| Base1024 | 10 | 0.8x | Slow | Dense CJK |

## Technical Details

### Why This Works

Base256 works identically in both modes because it's a power of 2 that perfectly divides a byte:

```
1 byte = 8 bits
base256 = 2^8

Chunked Mode:
  8 bits → 1 character (direct mapping)

Mathematical Mode:
  Number → base256 digits → same result!
```

### Encoding Modes

Both produce identical output:

**Chunked Mode** (default):
- Processes bits in 8-bit chunks
- Each chunk = 1 character
- Like traditional encoding

**Mathematical Mode**:
- Treats data as large number
- Converts to base-256
- Same result due to perfect division

### Configuration

```toml
[alphabets.base256_matrix]
chars = "ぁあぃいぅうぇえぉお..." # 256 characters
mode = "chunked"  # or "base_conversion" - identical!
```

## Use Cases

### 1. Matrix-Style Displays
Perfect for creating Matrix-like visual effects while maintaining data integrity.

### 2. Compact Unicode Encoding
Store binary data in Unicode text with zero expansion overhead.

### 3. Educational
Demonstrates the mathematical properties that make hex work in both modes.

### 4. Cool Factor
Because regular encodings are boring, and you want your data to look like it's from The Matrix.

## Performance

**Encoding**: ~Fast (similar to base64 chunked mode)
**Decoding**: ~Fast (HashMap lookup for non-ASCII chars)
**Memory**: 1:1 ratio (no expansion)

## Examples

Run the Matrix demo:
```bash
cargo run --example matrix_demo
```

Output shows:
- Matrix-style encoded messages
- Mode comparison (both identical)
- Efficiency analysis
- Visual demonstration

## Testing

All tests pass:
```bash
cargo test test_base256
# test_base256_matrix_like_hex ... ok
# test_base256_matrix_perfect_encoding ... ok
# test_base256_matrix_all_bytes ... ok
```

## Relation to Hexadecimal

See `docs/HEX_EXPLANATION.md` for the mathematical theory. Base256 extends this concept:

| Property | Hex | Base256 Matrix |
|----------|-----|----------------|
| Base | 16 (2^4) | 256 (2^8) |
| Bits/char | 4 | 8 |
| Expansion | 2x | 1x |
| Mode-independent | ✅ | ✅ |
| Visual style | Boring | Matrix |

## Future Enhancements

1. **Animation Support**: Output that actually "rains" down like The Matrix
2. **Color Codes**: Terminal color support for green Matrix effect
3. **Streaming Visualization**: Real-time encoding with visual effects
4. **Alternative Styles**: Swap character sets for different visual themes

## Conclusion

base256_matrix is not just an encoding - it's a statement. It proves that efficient encodings don't have to be boring, and that you can have your Matrix cake and encode it too.

Wake up, Neo. The Matrix has your data... and now it looks cool.

---

*"I know base256 kung fu."* - Neo, probably
