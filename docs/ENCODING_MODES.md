# Encoding Modes in base-d

base-d supports two fundamentally different encoding algorithms, each optimized for different use cases.

## Mathematical Base Conversion (default)

### How It Works

1. Interpret the entire input as one big-endian integer
2. Convert that number to the target base using division/modulo
3. Map remainders to dictionary characters

### Example: "Hi" with base-64

```
'H' = 72, 'i' = 105
Combined number = 72 × 256 + 105 = 18,537

Convert 18,537 to base-64:
  18,537 ÷ 64 = 289 remainder 33  → 'h'
     289 ÷ 64 = 4   remainder 33  → 'h'
       4 ÷ 64 = 0   remainder 4   → 'E'

Result: Ehh (LSB first, then reversed)
```

### Characteristics

✅ Works with ANY dictionary size (52, 7, 100, etc.)
✅ No padding needed
✅ Elegant and mathematically pure
✅ Leading zeros are preserved
❌ Not compatible with RFC standards
❌ Slightly variable encoding length

### Best For

- Playing cards (52 characters)
- DNA sequences (4 characters)
- Custom emoji dictionaries
- Any creative/esoteric encoding

## Bit-Chunking (RFC-compatible)

### How It Works

1. Process input in fixed-width bit groups
2. Each group maps directly to one output character
3. Pad with special character if needed

### Example: "Hi" with base-64

```
'H' = 01001000  'i' = 01101001

Split into 6-bit chunks:
  010010 000110 1001??  (pad last group)
     18      6     36

Map to dictionary:
  18 → 'S'
   6 → 'G'
  36 → 'k'
   + → '=' (padding)

Result: SGk=
```

### Characteristics

✅ RFC 4648 compatible (base64, base32, base16)
✅ Streamable (process chunks independently)
✅ Constant encoding overhead
✅ Industry standard for data transport
❌ Requires power-of-2 dictionary sizes
❌ Needs padding character

### Best For

- Standard base64 encoding
- Base32, base16 (hex)
- Any RFC-compliant encoding
- Interoperability with existing tools

## Comparison

| Feature | Mathematical | Chunked |
|---------|-------------|---------|
| Dictionary size | Any | Must be power of 2 |
| Padding | No | Yes (optional) |
| Output length | Variable | Predictable |
| Leading zeros | Preserved | N/A |
| Streaming | No | Yes |
| RFC compatible | No | Yes |
| Use case | Creative/custom | Standards compliance |

## Configuration

Choose the mode in `dictionaries.toml`:

```toml
[dictionaries.my_dictionary]
chars = "ABC..."
mode = "base_conversion"  # or "chunked"
padding = "="  # optional, only for chunked mode
```

## Examples

```bash
# Mathematical mode (cards)
echo "Data" | base-d -e cards
# Output: 🃎🃊🃍🃖🂺

# Chunked mode (RFC base64)
echo "Data" | base-d -e base64
# Output: RGF0YQo=

# Same dictionary, different mode
echo "Data" | base-d -e base64_math
# Output: BEF0YQo= (no padding, different encoding)
```
