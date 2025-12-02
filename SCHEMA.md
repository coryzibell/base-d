# Schema Encoding Specification

Schema encoding is a compact wire protocol designed for LLM-to-LLM communication. It transforms JSON into a binary-packed, display-safe format that:

- **Parser-inert**: Uses Egyptian hieroglyphs as delimiters (`𓍹...𓍺`) that syntax highlighters ignore
- **Display-safe**: 96-character alphabet of box-drawing and geometric shapes (no ASCII, no confusables)
- **Self-describing**: Schema metadata embedded in the binary header
- **Dense**: Column-oriented storage with optional compression

## Overview

The encoding pipeline:

```
JSON → Intermediate Representation → Binary → [Compress] → Display96 → Framed
```

Decoding reverses this:

```
Framed → Display96 → [Decompress] → Binary → Intermediate Representation → JSON
```

## Wire Format

Encoded data is wrapped in Egyptian hieroglyph quotation marks:

```
𓍹{payload}𓍺
```

- **Frame Start**: `𓍹` (U+13379 EGYPTIAN HIEROGLYPH V011A)
- **Frame End**: `𓍺` (U+1337A EGYPTIAN HIEROGLYPH V011B)
- **Payload**: Base-96 encoded binary using display96 alphabet

### Example

Input JSON:
```json
{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]}
```

Encoded output:
```
𓍹╣◟╥◕◝▰◣◥▟╺▖◘▰◝▤◀╧𓍺
```

### Display96 Alphabet

A 96-character alphabet curated for maximum display compatibility:

**Character Ranges:**
- Box Drawing (heavy/double only): U+2501–U+257B (44 chars)
- Block Elements (full block + quadrants): U+2588–U+259F (11 chars)
- Geometric Shapes (solid filled only): U+25A0–U+25FF (41 chars)

**Properties:**
- Visually distinct
- Cross-platform safe
- No blanks or ASCII confusables
- High contrast (solid fills or heavy lines)
- Size-independent rendering

Example characters: `━`, `┃`, `┏`, `█`, `▀`, `■`, `▲`, `◆`, `●`

## Binary Format

The binary payload consists of:

```
[compression_prefix][header][values]
```

### Compression Prefix (1 byte)

The first byte indicates the compression algorithm:

| Byte | Algorithm |
|------|-----------|
| 0x00 | None (uncompressed) |
| 0x01 | Brotli (default level 6) |
| 0x02 | LZ4 (default level 6) |
| 0x03 | Zstd (default level 6) |

All other bytes are invalid and will produce an error.

### Header Structure

The header is self-describing and variable-length:

```
[flags: u8]
[root_key?: varint_string]  // if FLAG_HAS_ROOT_KEY set
[row_count: varint]
[field_count: varint]
[field_types: 4-bit packed]
[field_names: varint_strings]
[null_bitmap?: bytes]        // if FLAG_HAS_NULLS set
```

#### Flags (1 byte)

| Bit | Flag | Description |
|-----|------|-------------|
| 0 | `FLAG_TYPED_VALUES` | Per-value type tags (reserved, not currently used) |
| 1 | `FLAG_HAS_NULLS` | Null bitmap present |
| 2 | `FLAG_HAS_ROOT_KEY` | Root key in header (e.g., `"users"` for `{"users":[...]}`) |
| 3-7 | Reserved | Must be 0 |

#### Field Types (4-bit tags)

Each field's type is encoded as a 4-bit tag:

| Tag | Type | Description |
|-----|------|-------------|
| 0 | U64 | Unsigned 64-bit integer (varint encoded) |
| 1 | I64 | Signed 64-bit integer (zigzag varint) |
| 2 | F64 | 64-bit float (IEEE 754, 8 bytes) |
| 3 | String | UTF-8 string (varint length + bytes) |
| 4 | Bool | Boolean (single bit in packed byte) |
| 5 | Null | Null value (no storage, tracked in null bitmap) |
| 6 | Array | Homogeneous array (element type follows) |
| 7 | Any | Mixed-type value (reserved) |

Field types are packed 2 per byte (upper 4 bits, lower 4 bits).

#### Root Key

If `FLAG_HAS_ROOT_KEY` is set, a varint-length string follows the flags byte. This represents the top-level key for single-array JSON like `{"users":[...]}`.

#### Row Count and Field Count

Both are encoded as varints (variable-length integers).

#### Field Names

Each field name is encoded as:
```
[length: varint][utf8_bytes]
```

For nested objects, fields are flattened with dotted notation:
- `user.profile.name` → `"user.profile.name"`

#### Null Bitmap

If `FLAG_HAS_NULLS` is set, a bitmap follows the field names. Each bit represents whether a value is null:

```
bitmap_size = (row_count * field_count + 7) / 8  // Round up to byte boundary
```

Bit position: `row * field_count + field_index`

- Bit = 1: value is null
- Bit = 0: value is non-null

### Value Encoding

Values are stored in **row-major order** (field1, field2, field1, field2, ...):

#### Integers (U64, I64)

- **U64**: Varint encoding (MSB continuation bit)
- **I64**: Zigzag encoding then varint (converts negatives to small positive values)

Varint format:
```
- 0x00-0x7F: 1 byte (value as-is)
- 0x80+: MSB set means more bytes follow
```

Example:
- `42` → `0x2A` (1 byte)
- `300` → `0xAC 0x02` (2 bytes)

Zigzag (for I64):
```
zigzag(n) = (n << 1) ^ (n >> 63)
```

#### Floats (F64)

8 bytes, IEEE 754 double-precision, little-endian.

#### Strings

```
[length: varint][utf8_bytes]
```

Empty strings have length 0 and no bytes.

#### Booleans

Packed 8 booleans per byte. Within each row, booleans are collected and packed together.

#### Arrays

Arrays are encoded as:
```
[element_count: varint][element1][element2]...
```

- **Homogeneous arrays**: Element type declared in field definition
- **Single-element arrays**: Unwrapped to the single value (not stored as array)
- **Null elements**: Not currently supported

#### Null Values

Null values are tracked in the null bitmap and occupy zero bytes in the values section.

## CLI Reference

### Encode

```bash
base-d schema [OPTIONS] [FILE]
```

**Options:**
- `-c, --compress <ALGO>`: Compression algorithm (brotli, lz4, zstd)
- `-o, --output <FILE>`: Output file (default: stdout)

**Examples:**
```bash
# Encode from stdin
echo '{"id":1,"name":"alice"}' | base-d schema

# Encode file with compression
base-d schema -c brotli data.json

# Encode and save to file
base-d schema -o encoded.txt data.json

# Encode with zstd compression
cat large.json | base-d schema -c zstd > compressed.txt
```

### Decode

```bash
base-d schema -d [OPTIONS] [FILE]
```

**Options:**
- `-d, --decode`: Decode mode
- `-p, --pretty`: Pretty-print JSON output
- `-o, --output <FILE>`: Output file (default: stdout)

**Examples:**
```bash
# Decode from stdin
echo '𓍹╣◟╥◕◝▰◣◥▟╺▖◘▰◝▤◀╧𓍺' | base-d schema -d

# Decode with pretty output
base-d schema -d --pretty encoded.txt

# Decode and save to file
base-d schema -d -o output.json encoded.txt

# Decode compressed data (auto-detected)
cat compressed.txt | base-d schema -d --pretty
```

## Limitations

Current implementation has the following constraints:

### Root Primitives Not Supported

Only JSON objects and arrays of objects are supported. Primitives at the root level will fail:

```json
// ❌ Not supported
"hello"
42
true
null

// ✓ Supported
{"value": "hello"}
[{"value": 42}]
```

### Null Array Elements Not Supported

Arrays cannot contain null elements:

```json
// ❌ Not supported
{"tags": [1, null, 3]}

// ✓ Supported
{"tags": [1, 2, 3]}
```

Use object fields for nullable values:

```json
{"score": null}  // ✓ Supported via null bitmap
```

### Single-Element Arrays Unwrap

Single-element arrays are unwrapped to the element value:

```json
// Input
{"tags": [42]}

// Stored as
{"tags": 42}
```

This is a space optimization. Multi-element arrays behave normally:

```json
{"tags": [1, 2, 3]}  // Stored as array
```

## Performance Characteristics

- **Encoding**: O(n) where n = total field count across all rows
- **Decoding**: O(n) with single-pass parsing
- **Memory**: Constant overhead for header, linear for values
- **Compression**: Optional, adds CPU cost but reduces wire size

Compression effectiveness varies by data:
- **Small objects** (<100 bytes): Often expands due to overhead
- **Medium objects** (100-1000 bytes): 1.2-2.5x reduction typical
- **Large objects** (>1KB): 2-5x reduction typical
- **Repetitive data**: Up to 10x reduction

Brotli offers best compression, LZ4 offers best speed, Zstd balances both.

## Design Rationale

### Why Egyptian Hieroglyphs?

Frame delimiters need to be:
1. **Parser-inert**: Ignored by syntax highlighters and code parsers
2. **Visually distinctive**: Clearly mark encoded content
3. **Unlikely to collide**: Rare in normal text

Egyptian hieroglyphs meet all criteria. They're treated as "other" by most parsers and immediately signal "special encoding."

### Why Display96?

Base-96 balances density with display safety:
- **Base-64**: Standard, but uses ASCII (parser-visible, confusable)
- **Base-85**: Better density, still ASCII
- **Base-100**: Good density (emoji), but rendering issues
- **Base-96**: Best balance of density and visual reliability

Display96 uses only box-drawing, blocks, and geometric shapes—characters that:
- Render consistently across fonts and platforms
- Have no semantic meaning in programming languages
- Are visually distinct from each other
- Work in fixed-width and proportional fonts

### Why Column-Oriented?

Schema encoding stores data in columns (all values for field 1, then all for field 2) for several reasons:

1. **Compression**: Same-type values compress better together
2. **Type safety**: All values match declared field type
3. **Null handling**: Bitmap efficiently tracks sparse nulls
4. **Size**: No repeated field names (stored once in header)

Trade-off: Requires parsing full header before accessing any values.

## Future Enhancements

Planned improvements tracked in the roadmap:

- **Format specification repository**: Separate spec to enable multi-language implementations
- **Nested object support**: Full object nesting without flattening
- **Nullable array elements**: Support `[1, null, 3]`
- **Incremental decoding**: Stream values without full header parse
- **Additional compression**: Support Snappy and LZMA
- **Root primitive support**: Allow top-level strings/numbers/bools

## Examples

### Simple Object

Input:
```json
{"id":1,"name":"alice","score":95.5}
```

Binary structure:
```
Flags: 0x00 (no nulls, no root key)
Row count: 1
Field count: 3
Field types: [U64, String, F64]
Field names: ["id", "name", "score"]
Values:
  - U64: 1 (varint)
  - String: "alice" (length 5 + bytes)
  - F64: 95.5 (8 bytes IEEE 754)
```

### Array of Objects

Input:
```json
{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]}
```

Binary structure:
```
Flags: 0x04 (FLAG_HAS_ROOT_KEY)
Root key: "users"
Row count: 2
Field count: 2
Field types: [U64, String]
Field names: ["id", "name"]
Values:
  - Row 0: U64(1), String("alice")
  - Row 1: U64(2), String("bob")
```

### With Nulls

Input:
```json
{"name":"alice","age":null,"active":true}
```

Binary structure:
```
Flags: 0x02 (FLAG_HAS_NULLS)
Row count: 1
Field count: 3
Field types: [String, Null, Bool]
Field names: ["name", "age", "active"]
Null bitmap: [0b00000010] (bit 1 set for field index 1)
Values:
  - String("alice")
  - (null - no bytes)
  - Bool(true)
```

## Library API

See [API.md](docs/API.md) for complete library usage. Quick example:

```rust
use base_d::{encode_schema, decode_schema};

// Encode
let json = r#"{"users":[{"id":1,"name":"alice"}]}"#;
let encoded = encode_schema(json, None)?;
println!("{}", encoded); // 𓍹...𓍺

// Decode
let decoded = decode_schema(&encoded, false)?;
assert_eq!(json, decoded);
```

## See Also

- [README.md](README.md) - Main base-d documentation
- [API.md](docs/API.md) - Library API reference
- [COMPRESSION.md](docs/COMPRESSION.md) - Compression guide
- [PERFORMANCE.md](docs/PERFORMANCE.md) - Performance benchmarks

---

**Note:** base-d is the reference implementation. A standalone format specification repository is planned to enable implementations in other languages.
