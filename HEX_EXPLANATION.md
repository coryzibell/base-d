# Hexadecimal and base-d: A Special Case

## TL;DR

**Hexadecimal works with BOTH encoding modes and produces identical output!**

This is a special mathematical property of bases that evenly divide 8 bits.

## The Question

"How does hexadecimal work? Is it different from the modes we support?"

## The Answer

Hexadecimal is **base-16**, and it's special because both of our encoding modes produce **identical output**!

### Why?

```
1 byte = 8 bits
base-16 = 2^4 (4 bits per character)
8 ÷ 4 = 2 characters per byte (perfect division!)
```

When the bits-per-byte divides evenly by bits-per-character, the mathematical base conversion and bit-chunking algorithms converge to the same result.

## Example: "Hi" (bytes: 0x48, 0x69)

### Chunked Mode (Traditional Hex)
```
Byte 1: 0x48 = 01001000
  Split: 0100 | 1000
         4   |  8
  Result: "48"

Byte 2: 0x69 = 01101001
  Split: 0110 | 1001
         6   |  9
  Result: "69"

Final: "4869"
```

### Mathematical Mode
```
Number = (72 × 256) + 105 = 18,537

Convert to base-16:
  18537 ÷ 16 = 1158 rem 9 → '9'
   1158 ÷ 16 = 72   rem 6 → '6'
     72 ÷ 16 = 4    rem 8 → '8'
      4 ÷ 16 = 0    rem 4 → '4'

Reverse: "4869"
```

**Both produce "4869"!**

## The Magic Formula

Modes produce identical output when:
```
8 (bits per byte) % log₂(base) == 0
```

### Bases Where This Works

| Base | log₂ | 8 % log₂ | Match? | Example |
|------|------|----------|--------|---------|
| 2    | 1    | 0        | ✓      | binary: 8 chars/byte |
| 4    | 2    | 0        | ✓      | DNA: 4 chars/byte |
| 16   | 4    | 0        | ✓      | hex: 2 chars/byte |
| 256  | 8    | 0        | ✓      | identity: 1 char/byte |
| 52   | ~5.7 | ~2.3     | ✗      | cards: different! |
| 64   | 6    | 2        | ✗      | base64: different! |

## Testing in base-d

```bash
# All three produce identical output:
echo -n "Hi" | xxd -p                    # 4869
echo -n "Hi" | base-d -a hex             # 4869  (chunked)
echo -n "Hi" | base-d -a hex_math        # 4869  (mathematical)

# Verify they all round-trip correctly:
echo -n "Test" | base-d -a hex | base-d -a hex -d
# Output: Test ✓
```

## Conclusion

**Hexadecimal is NOT different from our modes** - it works perfectly with both! 

This is a beautiful mathematical property where:
- Power-of-2 bases that evenly divide bytes
- Produce identical output regardless of algorithm
- Can use either mode with identical results

For bases like **cards (52)** or **base64 (64 with padding)**, the modes differ, which is why we need both!

## In base-d Configuration

```toml
# Either mode works identically for hex!
[alphabets.hex]
chars = "0123456789abcdef"
mode = "chunked"      # or "base_conversion" - same result!
```
