# base-d Architecture Diagram

## Project Overview

Universal multi-dictionary encoder: binary data to 33+ dictionaries (RFC standards, emoji, ancient scripts).

**Version:** 3.0.17
**Edition:** Rust 2024
**Total LOC:** ~33,400 (src/)

---

## Directory Structure

```
base-d/
├── Cargo.toml              # Package manifest, features (simd default)
├── dictionaries.toml       # Built-in dictionary definitions
├── src/
│   ├── main.rs             # CLI binary entry (8 LOC)
│   ├── lib.rs              # Library root, public API (407 LOC)
│   ├── prelude.rs          # Common re-exports
│   ├── convenience.rs      # High-level combo functions
│   ├── bench.rs            # Benchmarking utilities
│   ├── tests.rs            # Integration tests
│   │
│   ├── core/               # Domain primitives
│   │   ├── config.rs       # DictionaryConfig, EncodingMode, Registry (655 LOC)
│   │   └── dictionary.rs   # Dictionary struct, builder pattern (698 LOC)
│   │
│   ├── encoders/           # Encoding/decoding implementations
│   │   ├── algorithms/
│   │   │   ├── radix.rs    # True base conversion (114 LOC)
│   │   │   ├── chunked.rs  # RFC 4648 chunked mode (197 LOC)
│   │   │   ├── byte_range.rs # 1:1 byte mapping (151 LOC)
│   │   │   ├── errors.rs   # DecodeError, DictionaryNotFoundError (406 LOC)
│   │   │   └── schema/     # Structured encoding subsystem
│   │   │       ├── types.rs          # IR types, SchemaValue (612 LOC)
│   │   │       ├── parsers/          # JSON, Markdown parsers
│   │   │       │   ├── json.rs       # JSON -> IR (910 LOC)
│   │   │       │   └── markdown_doc.rs # Markdown -> IR (623 LOC)
│   │   │       ├── serializers/      # IR -> output formats
│   │   │       │   └── json.rs       # IR -> JSON (612 LOC)
│   │   │       ├── binary_packer.rs  # IR -> binary (311 LOC)
│   │   │       ├── binary_unpacker.rs # binary -> IR (424 LOC)
│   │   │       ├── fiche.rs          # Model-readable format (2440 LOC)
│   │   │       ├── fiche_analyzer.rs # Auto-detection (253 LOC)
│   │   │       ├── display96.rs      # Safe wire format (142 LOC)
│   │   │       ├── frame.rs          # Delimiters (303 LOC)
│   │   │       └── compression.rs    # Brotli/LZ4/Zstd (266 LOC)
│   │   │
│   │   └── streaming/      # Memory-efficient large file processing
│   │       ├── encoder.rs  # StreamingEncoder (319 LOC)
│   │       ├── decoder.rs  # StreamingDecoder (361 LOC)
│   │       └── hasher.rs   # StreamingHasher (254 LOC)
│   │
│   ├── features/           # Optional capabilities
│   │   ├── compression.rs  # 7 algorithms: gzip, brotli, zstd, lz4, snappy, xz, deflate (233 LOC)
│   │   ├── hashing.rs      # 15 algorithms: SHA, BLAKE, xxHash, etc. (616 LOC)
│   │   └── detection.rs    # Dictionary auto-detection (409 LOC)
│   │
│   ├── simd/               # SIMD acceleration (~15,400 LOC total)
│   │   ├── mod.rs          # Unified entry points (676 LOC)
│   │   ├── variants.rs     # Dictionary variant identification (727 LOC)
│   │   ├── translate.rs    # Unicode translation tables (484 LOC)
│   │   ├── generic/        # GenericSimdCodec (2281 LOC)
│   │   ├── lut/            # Lookup table codecs
│   │   │   ├── base16.rs   # (999 LOC)
│   │   │   ├── base32.rs   # (590 LOC)
│   │   │   ├── base64.rs   # (2510 LOC)
│   │   │   └── gapped.rs   # Gapped sequential (1685 LOC)
│   │   ├── x86_64/         # AVX2/SSSE3 implementations
│   │   │   └── specialized/ # base16, base32, base64, base256
│   │   └── aarch64/        # NEON implementations
│   │       └── specialized/ # base16, base32, base64, base256
│   │
│   └── cli/                # Command-line interface
│       ├── mod.rs          # CLI entry, command dispatch (74 LOC)
│       ├── args.rs         # Argument definitions (303 LOC)
│       ├── commands.rs     # Legacy command support (595 LOC)
│       ├── config.rs       # CLI config loading (148 LOC)
│       ├── global.rs       # Global options (25 LOC)
│       └── handlers/       # Command implementations
│           ├── encode.rs   # (155 LOC)
│           ├── decode.rs   # (119 LOC)
│           ├── detect.rs   # (21 LOC)
│           ├── hash.rs     # (78 LOC)
│           ├── schema.rs   # (38 LOC)
│           ├── fiche.rs    # (133 LOC)
│           ├── config.rs   # (131 LOC)
│           └── neo.rs      # Matrix effect (57 LOC)
│
├── benches/                # Criterion benchmarks
├── tests/                  # Integration tests
├── examples/               # Usage examples
└── docs/                   # Documentation
```

---

## Component Map

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              PUBLIC API (lib.rs)                            │
│  encode() / decode() / hash() / compress() / detect_dictionary()            │
│  encode_schema() / decode_schema() / encode_fiche() / decode_fiche()        │
│  DictionaryRegistry / Dictionary / StreamingEncoder / StreamingDecoder      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────────────────┐ │
│  │   core/    │  │ features/  │  │ encoders/  │  │        simd/           │ │
│  │            │  │            │  │            │  │                        │ │
│  │ Dictionary │  │ Hashing    │  │ Radix      │  │ x86_64: AVX2/SSSE3     │ │
│  │ Config     │  │ Compress   │  │ Chunked    │  │ aarch64: NEON          │ │
│  │ Registry   │  │ Detection  │  │ ByteRange  │  │                        │ │
│  │ Mode       │  │            │  │ Streaming  │  │ Specialized:           │ │
│  │            │  │            │  │ Schema     │  │  base16/32/64/256      │ │
│  │            │  │            │  │            │  │                        │ │
│  └────────────┘  └────────────┘  └────────────┘  │ Generic:               │ │
│                                                  │  GenericSimdCodec      │ │
│                                                  │  GappedSequentialCodec │ │
│                                                  │  SmallLutCodec         │ │
│                                                  │  Base64LutCodec        │ │
│                                                  └────────────────────────┘ │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        encoders/algorithms/schema/                    │   │
│  │                                                                       │   │
│  │   ┌─────────────┐     ┌──────────────┐     ┌──────────────────┐      │   │
│  │   │  Parsers    │     │  IR Layer    │     │   Serializers    │      │   │
│  │   │             │     │              │     │                  │      │   │
│  │   │  JSON       │ --> │ Schema-      │ --> │  JSON            │      │   │
│  │   │  Markdown   │     │ Header       │     │  Fiche           │      │   │
│  │   │  (Custom)   │     │ Values       │     │  (Custom)        │      │   │
│  │   └─────────────┘     └──────────────┘     └──────────────────┘      │   │
│  │                             │                      ↑                  │   │
│  │                             ↓                      │                  │   │
│  │                    ┌──────────────┐       ┌──────────────┐           │   │
│  │                    │ Binary Layer │       │ Frame Layer  │           │   │
│  │                    │ pack/unpack  │ <---> │ display96    │           │   │
│  │                    │              │       │ delimiters   │           │   │
│  │                    └──────────────┘       └──────────────┘           │   │
│  │                                                                       │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ↓
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CLI (cli/)                                     │
│                                                                             │
│   ┌─────────┐    ┌──────────────────────────────────────────────────────┐  │
│   │ main.rs │--->│                    cli/mod.rs                        │  │
│   └─────────┘    │  Cli struct -> Commands enum -> handler dispatch     │  │
│                  └──────────────────────────────────────────────────────┘  │
│                                        │                                    │
│         ┌──────────────────────────────┼──────────────────────────────┐    │
│         │              │               │               │              │    │
│         ↓              ↓               ↓               ↓              ↓    │
│    ┌─────────┐   ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐  │
│    │ encode  │   │ decode  │    │ schema  │    │  fiche  │    │  hash   │  │
│    │ handler │   │ handler │    │ handler │    │ handler │    │ handler │  │
│    └─────────┘   └─────────┘    └─────────┘    └─────────┘    └─────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Flow Diagrams

### Standard Encoding Flow

```
Input (bytes) ────┬──── [Dictionary Selection] ────────────────────────┐
                  │                                                     │
                  │     ┌─────────────────────────────────────────┐    │
                  │     │         Dictionary Mode?                │    │
                  │     └────────────┬────────────┬───────────────┘    │
                  │                  │            │           │        │
                  │               Radix       Chunked    ByteRange     │
                  │                  │            │           │        │
                  ↓                  ↓            ↓           ↓        │
         [SIMD Available?] ─────────┼────────────┼───────────┼────────┤
                  │                  │            │           │        │
            Yes   │   No             │            │           │        │
              ↓   ↓                  │            │           │        │
         ┌────────────┐              │            │           │        │
         │   SIMD     │              │            │           │        │
         │ Selection  │              │            │           │        │
         │ Cascade    │              │            │           │        │
         └────────────┘              │            │           │        │
              │                      │            │           │        │
              ↓                      ↓            ↓           ↓        │
         [Specialized] ──────> [radix.rs] ──> [chunked.rs] ──> [byte_range.rs]
              │                      │            │           │        │
              │                      └────────────┴───────────┘        │
              │                                   │                     │
              └───────────────────────────────────┼─────────────────────┘
                                                  ↓
                                            Output (String)
```

### Schema Encoding Pipeline

```
JSON String ───────────────────────────────────────────────────────────────────┐
      │                                                                        │
      ↓                                                                        │
┌─────────────────┐                                                            │
│   JsonParser    │  parse JSON to Intermediate Representation (IR)            │
│   (Input Layer) │                                                            │
└────────┬────────┘                                                            │
         │                                                                     │
         ↓                                                                     │
┌─────────────────┐                                                            │
│  IR (header +   │  SchemaHeader: fields, types, row_count, flags, nulls      │
│     values)     │  SchemaValue[]: flat row-major array                       │
└────────┬────────┘                                                            │
         │                                                                     │
         ↓                                                                     │
┌─────────────────┐                                                            │
│   pack()        │  IR -> binary bytes (VarInt encoding)                      │
│  Binary Layer   │                                                            │
└────────┬────────┘                                                            │
         │                                                                     │
         ↓                                                                     │
┌─────────────────┐                                                            │
│  compress?      │  Optional: Brotli/LZ4/Zstd with prefix byte                │
│                 │                                                            │
└────────┬────────┘                                                            │
         │                                                                     │
         ↓                                                                     │
┌─────────────────┐                                                            │
│  display96      │  Binary -> 96-char display-safe alphabet                   │
│  + frame        │  Wrapped in delimiters: U+1334 ... U+1334A                 │
└────────┬────────┘                                                            │
         │                                                                     │
         ↓                                                                     │
    Encoded Wire Format ───────────────────────────────────────────────────────┘
```

### SIMD Selection Cascade (x86_64)

```
encode_with_simd(data, dict)
         │
         ↓
    [Check AVX2/SSSE3]
         │
         ↓  (not available)
         └──────────────────────> return None (scalar fallback)
         │
         ↓  (available)
    [base == 64 && known variant?]
         │ Yes                     No
         ↓                          │
    encode_base64_simd()            │
         │                          ↓
         │               [base == 32 && known variant?]
         │                    │ Yes              No
         │                    ↓                   │
         │               encode_base32_simd()     │
         │                    │                   ↓
         │                    │         [base == 16 && standard hex?]
         │                    │               │ Yes              No
         │                    │               ↓                   │
         │                    │          encode_base16_simd()     │
         │                    │               │                   ↓
         │                    │               │         [base == 256 && ByteRange?]
         │                    │               │               │ Yes           No
         │                    │               │               ↓                │
         │                    │               │          encode_base256_simd() │
         │                    │               │               │                ↓
         │                    │               │               │     [GenericSimdCodec?]
         │                    │               │               │          │ Yes     No
         │                    │               │               │          ↓          │
         │                    │               │               │      generic.encode │
         │                    │               │               │          │          ↓
         │                    │               │               │          │  [GappedSequential?]
         │                    │               │               │          │       │ Yes    No
         │                    │               │               │          │       ↓         │
         │                    │               │               │          │   gapped.encode │
         │                    │               │               │          │       │         ↓
         │                    │               │               │          │       │   [SmallLut/Base64Lut?]
         │                    │               │               │          │       │        │ Yes    No
         │                    │               │               │          │       │        ↓         │
         │                    │               │               │          │       │    lut.encode    │
         │                    │               │               │          │       │        │         ↓
         └────────────────────┴───────────────┴───────────────┴──────────┴───────┴────────┴──> return None
```

---

## Module Boundaries

| Boundary | Inner Module | Outer Depends On | Contract |
|----------|--------------|------------------|----------|
| core | `dictionary`, `config` | lib.rs, encoders, cli | Dictionary, EncodingMode, DictionaryRegistry |
| encoders/algorithms | radix, chunked, byte_range | lib.rs (encode/decode) | encode_*(), decode_*() functions |
| encoders/algorithms/schema | parsers, serializers, binary, frame | lib.rs | InputParser, OutputSerializer traits |
| encoders/streaming | encoder, decoder, hasher | lib.rs | StreamingEncoder, StreamingDecoder |
| features | compression, hashing, detection | lib.rs, cli | Algorithm enums, compress/hash/detect |
| simd | x86_64, aarch64, generic, lut | encoders/algorithms | encode_with_simd(), decode_with_simd() |
| cli | handlers, args | main.rs | run() entry point |

---

## Entry Points

| Entry Point | Location | Type | Purpose |
|-------------|----------|------|---------|
| `main()` | `src/main.rs:8` | Binary | CLI application |
| `cli::run()` | `src/cli/mod.rs:57` | CLI | Command dispatch |
| `encode()` | `src/lib.rs:346` | Library | Primary encode API |
| `decode()` | `src/lib.rs:396` | Library | Primary decode API |
| `encode_schema()` | `src/encoders/algorithms/schema/mod.rs:70` | Library | Schema encoding |
| `decode_schema()` | `src/encoders/algorithms/schema/mod.rs:129` | Library | Schema decoding |
| `encode_fiche()` | `src/encoders/algorithms/schema/mod.rs:160` | Library | Fiche encoding |
| `decode_fiche()` | `src/encoders/algorithms/schema/mod.rs:281` | Library | Fiche decoding |
| `encode_with_simd()` | `src/simd/mod.rs:104` (x86), `288` (aarch64) | Internal | SIMD acceleration |

---

## External Dependencies

| Dependency | Purpose | Category |
|------------|---------|----------|
| num-bigint, num-traits, num-integer | Radix base conversion | Core |
| serde, serde_json, toml | Config/data serialization | Core |
| clap | CLI argument parsing | CLI |
| flate2, brotli, zstd, lz4, snap, xz2 | Compression algorithms | Features |
| sha2, sha3, blake2, blake3, md-5, twox-hash, crc, ascon-hash, k12 | Hashing algorithms | Features |
| rand | Random dictionary selection | Features |
| crossterm, terminal_size | CLI terminal effects | CLI |
| markdown | Markdown parsing | Schema |
| dirs | User config paths | Config |
| shellexpand | Path expansion | Config |
| hex | Hex encoding utilities | Utilities |

---

## Feature Flags

| Feature | Default | Effect |
|---------|---------|--------|
| `simd` | Yes | Enables SIMD acceleration (AVX2/SSSE3 on x86_64, NEON on aarch64) |

---

## Data Structures

### Core Types

```
Dictionary {
    chars: Vec<char>           // Character set
    char_to_index: HashMap     // Reverse lookup
    lookup_table: Option<[u8;256]>  // Fast ASCII lookup
    mode: EncodingMode         // Radix | Chunked | ByteRange
    padding: Option<char>
    start_codepoint: Option<u32>
}

DictionaryRegistry {
    dictionaries: HashMap<String, DictionaryConfig>
    compression: HashMap<String, CompressionConfig>
    settings: Settings
}

EncodingMode = Radix | Chunked | ByteRange
```

### Schema IR Types

```
IntermediateRepresentation {
    header: SchemaHeader
    values: Vec<SchemaValue>
}

SchemaHeader {
    row_count: usize
    fields: Vec<FieldDef>
    flags: u8
    root_key: Option<String>
    null_bitmap: Option<Vec<u8>>
}

FieldDef { name: String, field_type: FieldType }

FieldType = U64 | I64 | F64 | String | Bool | Null | Array(Box<FieldType>) | Any

SchemaValue = U64(u64) | I64(i64) | F64(f64) | String(String) | Bool(bool) | Null | Array(Vec<SchemaValue>)
```
