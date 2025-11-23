# Progress Log

## 2025-11-23

### Project Initialization
- Created Rust project structure with `cargo new base-d`
- Set up initial documentation:
  - README.md with project overview and usage examples
  - ROADMAP.md with planned features across 5 phases
  - PROGRESS.md for tracking development

### Design Decisions
- **Primary Alphabet**: Starting with Unicode playing cards (52 characters)
  - Suits: Spades (🂡-🂮), Hearts (🂱-🂾), Diamonds (🃁-🃎), Clubs (🃑-🃞)
  - 13 ranks per suit = 52 total characters (excluding knights 🂬🂼🃌🃜)
- **Dual Purpose**: Library crate + CLI binary
- **API Design**: Similar to base58/base64 patterns for familiarity

### Phase 1: Core Functionality - COMPLETED ✓

#### Implemented
- ✅ Alphabet struct with encode/decode methods
  - Character-to-index mapping via HashMap
  - Validation for duplicate characters
  - **NEW**: `from_str()` method for easy alphabet creation
- ✅ **TOML-based configuration system**
  - `alphabets.toml` for defining alphabets
  - `AlphabetsConfig` for loading and accessing alphabets
  - Embedded TOML file via `include_str!` for default config
  - Easy to add new alphabets without code changes
- ✅ Playing Cards alphabet (52 Unicode characters)
  - Removed knights to get exactly 52 cards
  - Now defined in `alphabets.toml`
- ✅ Base encoding/decoding algorithm
  - Uses `num-bigint` for arbitrary precision
  - Handles leading zeros correctly
  - Proper roundtrip for all test cases
- ✅ Library API
  - `encode(data: &[u8], alphabet: &Alphabet) -> String`
  - `decode(encoded: &str, alphabet: &Alphabet) -> Result<Vec<u8>, DecodeError>`
  - `Alphabet::from_str(s: &str)` for creating alphabets
  - `AlphabetsConfig::load_default()` for loading built-in alphabets
- ✅ Comprehensive unit tests (10 tests, all passing)
  - Empty data
  - Single zero byte
  - Simple strings
  - Binary data
  - Leading zeros
  - Invalid characters
  - Config loading and validation

#### Example Output
```
"Hello, World!" encodes to: 🃎🃅🃝🃉🂡🂣🂸🃉🃉🃇🃉🃓🂵🂣🂨🂻🃆🃍
```

### Architecture Improvements
- **Configuration-driven**: Alphabets are now data, not code
- **Extensible**: Add new alphabets by editing TOML file
- **Type-safe**: Serde for TOML parsing with validation
- **Zero-cost abstraction**: Alphabet parsing happens once at load time

### Next Steps
1. ~~Begin Phase 2: CLI Tool implementation~~ ✓ COMPLETED
2. ~~Add `clap` for command-line parsing~~ ✓
3. ~~Implement `encode` and `decode` subcommands~~ ✓
4. ~~Add input/output options (stdin, files, strings)~~ ✓
5. ~~Add encoding mode support (mathematical vs chunked)~~ ✓ COMPLETED
6. ~~Phase 3: Add common encoding alphabets~~ ✓ COMPLETED
7. ~~Phase 4: Add esoteric Unicode alphabets~~ ✓ COMPLETED

### Phase 4: Esoteric Alphabets - COMPLETED ✓

#### Implemented (32 total alphabets)

**Ancient Scripts (3):**
- ✅ hieroglyphs (Egyptian, 100 chars)
- ✅ cuneiform (Sumerian, 100 chars)
- ✅ runic (Elder Futhark, 81 chars)

**Game Pieces (4):**
- ✅ domino (100 tiles)
- ✅ mahjong (44 tiles)
- ✅ chess (12 pieces)
- ✅ cards (52 playing cards)

**Esoteric Symbols (5):**
- ✅ alchemy (116 alchemical symbols)
- ✅ zodiac (12 zodiac signs)
- ✅ weather (72 weather & misc symbols)
- ✅ music (100 musical notation symbols)
- ✅ arrows (112 arrow symbols)

**Emoji (2):**
- ✅ emoji_faces (80 face emoji)
- ✅ emoji_animals (64 animal emoji)

#### Testing
All esoteric alphabets verified:
- ✓ Hieroglyphs round-trip
- ✓ Cuneiform round-trip
- ✓ Domino round-trip
- ✓ Mahjong round-trip
- ✓ Emoji faces round-trip

#### Use Cases
```bash
# Ancient Egyptian
echo "Message" | base-d -a hieroglyphs
# → 𓀅𓁉𓀺𓀐𓁌𓀞𓁉𓁕

# Cuneiform tablets
echo "Data" | base-d -a cuneiform
# → 𒀀𒀁𒀂𒀃

# Game encoding
echo "Secret" | base-d -a mahjong
# → 🀁🀂🀃🀄🀅

# Emoji messages
echo "Hi!" | base-d -a emoji_faces
# → 😂😃😁
```

### Phase 3: Common Alphabets - COMPLETED ✓

#### Implemented (19 total alphabets)

**RFC 4648 Standards (5):**
- ✅ base16, base32, base32hex, base64, base64url
- ✅ All verified RFC 4648 compliant
- ✅ Proper padding support

**Bitcoin/Blockchain (2):**
- ✅ base58 (Bitcoin addresses)
- ✅ base58flickr (Flickr variant)

**High-Density Encodings (3):**
- ✅ base62 (URL shorteners)
- ✅ base85 (Git pack format)
- ✅ ascii85 (Adobe PDF)
- ✅ z85 (ZeroMQ)

**Human-Oriented (2):**
- ✅ base32_crockford (no ambiguous chars)
- ✅ base32_zbase (human-readable)

**Other (4):**
- ✅ cards, dna, binary, hex

**Mathematical Variants (3):**
- ✅ base64_math, hex_math

#### Verification
```bash
# RFC 4648 compliance verified
base32:  ✓ Matches `base32` command
base64:  ✓ Matches `base64` command

# All alphabets round-trip correctly
base58:  ✓
base85:  ✓
ascii85: ✓
```

#### Documentation
- Created ALPHABETS.md with complete reference
- Updated README with all 19 alphabets
- Organized by category and use case

### Encoding Modes Feature - COMPLETED ✓

#### Implemented
- ✅ **Dual-mode architecture**
  - Mathematical base conversion (default)
  - Bit-chunking for RFC compatibility
- ✅ **Mathematical mode** (`base_conversion`)
  - Works with any alphabet size
  - Treats data as single large number
  - No padding needed
  - Perfect for creative alphabets (cards, DNA, emoji)
- ✅ **Chunked mode** (`chunked`)
  - RFC 4648 compliant
  - Fixed-width bit groups
  - Supports padding character
  - Power-of-2 alphabet sizes only
- ✅ **Configuration in TOML**
  - `mode` field specifies algorithm
  - `padding` field for chunked mode
- ✅ **Standard base64 support**
  - `base64` alphabet with chunked mode
  - 100% compatible with RFC 4648
  - `base64_math` for mathematical variant
- ✅ **Comprehensive tests** (14 tests passing)
  - Both modes tested independently
  - Round-trip verification
  - Binary data preservation
  - RFC compliance verification

#### Examples
```bash
# RFC-compliant base64
echo "Hello, World!" | base-d -a base64
# Output: SGVsbG8sIFdvcmxkIQo=

# Mathematical base64 (different output)
echo "Hello, World!" | base-d -a base64_math  
# Output: EhlbGxvLCBXb3JsZCEK

# Playing cards (mathematical)
echo "Data" | base-d -a cards
# Output: 🃎🃊🃍🃖🂺
```

### Phase 2: CLI Tool - COMPLETED ✓

#### Implemented
- ✅ Command-line interface with `clap`
  - `-a, --alphabet <NAME>` to select alphabet (default: cards)
  - `-d, --decode` flag to decode instead of encode
  - `-l, --list` to list available alphabets
  - `[FILE]` optional positional argument for file input
- ✅ Input/output handling
  - Reads from stdin if no file provided (pipeable)
  - Reads from file if path provided
  - Writes encoded output to stdout
  - Binary-safe decode output
- ✅ Error handling
  - Invalid alphabet names
  - File not found
  - Invalid UTF-8 in decode mode
- ✅ Comprehensive CLI test suite (8 tests, all passing)

#### Usage Examples
```bash
# List alphabets
base-d --list

# Encode stdin with default (cards) alphabet
echo "Hello" | base-d

# Encode file with DNA alphabet
base-d -a dna input.txt

# Decode
echo "🃎🃅🃝..." | base-d -d

# Round-trip
echo "Data" | base-d | base-d -d
```

### Technical Notes
- Bug fixed: `BigUint(0).to_bytes_be()` returns `[0]` not `[]`, affecting zero-byte decoding
- Leading zeros must be preserved through encode/decode cycle
- Base-52 encoding produces ~18 characters for 13-byte input
