# base-d

![base-d demo](assets/impressive-based.gif)

[![Crates.io](https://img.shields.io/crates/v/base-d.svg)](https://crates.io/crates/base-d)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

**Turns bytes into anything.** Playing cards, hieroglyphs, emoji, RFC base64 — same engine, your choice of alphabet.

---

## Why base-d?

You probably have `base64`, `sha256sum`, `crc32` as separate tools. base-d does all of it:

- **Encode** with 45+ dictionaries (or define your own)
- **Hash** with 26 algorithms — SHA-256, BLAKE3, CRC32, xxHash3
- **Compress** with gzip, zstd, brotli, lz4, snappy, lzma
- **Stream** multi-GB files with constant 4KB memory
- **Detect** which encoding was used automatically

One tool. SIMD-accelerated. **7.4 GiB/s** decode, **500 MiB/s** encode.

---

## Pick your path

**Want to mess around?**
```bash
cargo install base-d
base-d neo
```
> *Wake up, Neo...*

**Want the CLI?** → [CLI Quick Start](#cli-quick-start) | [Full CLI docs](docs/CLI.md)

**Want the library?** → [Library Quick Start](#library-quick-start) | [Full API docs](docs/API.md)

---

## CLI Quick Start

```bash
# Encode with playing cards (default dictionary)
echo "secret" | base-d encode cards
# 🂡🂢🂣🂤🂥🂦

# RFC base64
echo "hello" | base-d encode base64
# aGVsbG8=

# Hieroglyphics, because why not
echo "pharaoh" | base-d encode hieroglyphics

# Word-based encoding (BIP-39 seed phrases)
echo "secret" | base-d encode bip39
# abandon absorb morning...

# Hash a file
base-d hash sha256 myfile.bin

# Compress + encode in one shot
base-d encode base64 --compress zstd < bigfile.json

# Auto-detect and decode
echo "aGVsbG8=" | base-d decode --detect
```

[More CLI examples →](docs/CLI.md)

---

## Library Quick Start

```rust
use base_d::{encode, decode, Dictionary, DictionaryRegistry};

// Basic encoding
let registry = DictionaryRegistry::with_builtins();
let dict = registry.get("base64").unwrap();
let encoded = encode(b"hello world", dict);
let decoded = decode(&encoded, dict)?;

// Streaming for large data
use base_d::streaming::{StreamEncoder, StreamDecoder};
let mut encoder = StreamEncoder::new(dict);
encoder.update(chunk1)?;
encoder.update(chunk2)?;
let result = encoder.finalize()?;
```

[More library examples →](docs/API.md)

---

## How it works

### The core idea

base-d is a universal encoder. It converts bytes into symbols using *dictionaries* — lookup tables that map values to characters. The dictionary is the only thing that changes between "serious RFC base64" and "playing cards."

```
bytes → [dictionary] → symbols
```

### Three encoding modes

| Mode | How it works | Best for |
|------|--------------|----------|
| **Mathematical** | Treats data as one big number, converts to target base | Any dictionary size, compact output |
| **Chunked** | RFC 4648 style, processes fixed bit groups | Standards compliance (base64, base32) |
| **Byte Range** | 1:1 byte-to-symbol mapping | base256, emoji, visual encodings |

[Deep dive: Encoding modes →](docs/ENCODING_MODES.md)

### Dictionaries

45+ built-in dictionaries across categories:

| Category | Examples |
|----------|----------|
| **Standards** | base64, base32, base16, base58, base85 |
| **Ancient scripts** | Hieroglyphics, cuneiform, Elder Futhark |
| **Games** | Playing cards, mahjong, chess pieces, dominos |
| **Esoteric** | Alchemy symbols, zodiac, weather, musical notation |
| **Modern** | Emoji, Matrix-style katakana, CJK base1024 |
| **Word lists** | BIP-39, Diceware, EFF, PGP, NATO, Pokemon, Klingon |

[Full dictionary list →](docs/DICTIONARIES.md) | [Create your own →](docs/CUSTOM_DICTIONARIES.md)

### Performance

SIMD-accelerated with runtime detection:
- **x86_64**: AVX2, SSSE3 with specialized RFC dictionary paths
- **ARM**: NEON with equivalent optimizations
- **Fallback**: Portable LUT-based implementation

| Operation | Throughput |
|-----------|------------|
| base64 decode (AVX2) | 7.4 GiB/s |
| base64 encode (AVX2) | 500 MiB/s |
| Arbitrary dictionary | 50-200 MiB/s |

[SIMD internals →](docs/SIMD.md) | [Benchmarks →](docs/BENCHMARKING.md)

### Schema encoding (fiche)

Structured data encoding that preserves type information:

```bash
echo '{"name": "Neo", "age": 30}' | base-d fiche encode
# Compact binary with recoverable structure

base-d fiche decode < encoded.bin
# {"name": "Neo", "age": 30}
```

[Schema deep dive →](docs/SCHEMA.md)

---

## Documentation

| Topic | Description |
|-------|-------------|
| [API Reference](docs/API.md) | Library usage, examples, types |
| [CLI Reference](docs/CLI.md) | Commands, flags, workflows |
| [Encoding Modes](docs/ENCODING_MODES.md) | Mathematical vs chunked vs byte-range |
| [Dictionaries](docs/DICTIONARIES.md) | Built-in dictionaries reference |
| [Custom Dictionaries](docs/CUSTOM_DICTIONARIES.md) | Define your own alphabets |
| [Compression](docs/COMPRESSION.md) | Compress-then-encode pipeline |
| [Hashing](docs/HASHING.md) | 26 hash algorithms |
| [Streaming](docs/STREAMING.md) | Memory-efficient large file processing |
| [SIMD](docs/SIMD.md) | Performance internals |
| [Schema/Fiche](docs/SCHEMA.md) | Structured data encoding |
| [Detection](docs/DETECTION.md) | Auto-detect encoding format |
| [Neo Mode](docs/NEO.md) | The Matrix easter egg |

---

## Install

```bash
# From crates.io
cargo install base-d

# From source
git clone https://github.com/coryzibell/base-d
cd base-d
cargo build --release
```

---

## License

MIT OR Apache-2.0
