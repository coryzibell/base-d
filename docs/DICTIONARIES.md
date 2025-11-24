# Dictionary Reference

Complete reference for all numerous built-in dictionaries in base-d.

## Quick Reference Table

| Name | Base | Mode | Use Case | RFC |
|------|------|------|----------|-----|
| **base16** | 16 | chunk | Hex (uppercase) | RFC 4648 |
| **hex** | 16 | chunk | Hex (lowercase) | - |
| **base32** | 32 | chunk | Data encoding | RFC 4648 |
| **base32hex** | 32 | chunk | Extended hex dictionary | RFC 4648 |
| **base64** | 64 | chunk | Standard base64 | RFC 4648 |
| **base64url** | 64 | chunk | URL-safe base64 | RFC 4648 |
| **base58** | 58 | math | Bitcoin addresses | - |
| **base58flickr** | 58 | math | Flickr short URLs | - |
| **base62** | 62 | math | URL shorteners | - |
| **base85** | 85 | math | Git, Mercurial | - |
| **ascii85** | 85 | math | Adobe PDF, PostScript | btoa |
| **z85** | 84 | math | ZeroMQ | ZeroMQ spec |
| **base32_crockford** | 32 | math | Human-readable IDs | Crockford |
| **base32_zbase** | 32 | math | Human-oriented | z-base-32 |
| **base100** | 256 | range | Emoji encoding | base💯 |
| **cards** | 52 | math | Fun encoding | - |
| **dna** | 4 | math | Genetic sequences | - |
| **binary** | 2 | math | Binary | - |
| **base64_math** | 64 | math | Math variant | - |
| **hex_math** | 16 | math | Math variant | - |

## Detailed Descriptions

### RFC 4648 Standards

#### base16
```
Dictionary: 0123456789ABCDEF
Padding:  No
Example:  "Hi" → "4869"
```
Standard hexadecimal encoding (uppercase). Power-of-2 base.

#### base32
```
Dictionary: ABCDEFGHIJKLMNOPQRSTUVWXYZ234567
Padding:  =
Example:  "Hi" → "JBQWY==="
```
RFC 4648 standard base32. Good balance of efficiency and readability.

#### base32hex
```
Dictionary: 0123456789ABCDEFGHIJKLMNOPQRSTUV
Padding:  =
Example:  "Hi" → "91GOR==="
```
RFC 4648 extended hex dictionary variant. Preserves sort order.

#### base64
```
Dictionary: A-Z, a-z, 0-9, +, /
Padding:  =
Example:  "Hi" → "SGk="
```
Standard base64. Most common encoding for binary-to-text.

#### base64url
```
Dictionary: A-Z, a-z, 0-9, -, _
Padding:  =
Example:  "Hi" → "SGk="
```
URL and filename-safe variant. Uses - and _ instead of + and /.

### Bitcoin & Blockchain

#### base58
```
Dictionary: 123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
Example:  "Hi" → "JxF"
```
Bitcoin addresses. Removes confusing characters: 0, O, I, l.

#### base58flickr
```
Dictionary: 123456789abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ
Example:  "Hi" → "jXf"
```
Flickr's variant with different case ordering.

### High-Density Encodings

#### base62
```
Dictionary: 0-9, A-Z, a-z
Example:  "Hi" → "5O"
```
Alphanumeric only. Popular for URL shorteners.

#### base85
```
Dictionary: 0-9, A-Z, a-z, plus special chars
Example:  "Hi" → "BOq"
```
Used by Git for pack files. More efficient than base64.

#### ascii85
```
Dictionary: ! through u (ASCII 33-117)
Example:  "Hi" → "BOq"
```
Adobe PDF and PostScript encoding. Also called "btoa".

#### z85
```
Dictionary: 0-9, a-z, A-Z, and selected punctuation
Example:  "Hi" → "xK#0"
```
ZeroMQ's string-safe encoding.

### Human-Oriented

#### base32_crockford
```
Dictionary: 0-9, A-H, J-K, M-N, P-T, V-Z (no I, L, O, U)
Example:  "Hi" → "48B"
```
Douglas Crockford's base32. Removes ambiguous characters.

#### base32_zbase
```
Dictionary: ybndrfg8ejkmcpqxot1uwisza345h769
Example:  "Hi" → "nxny"
```
Designed for human readability. No ambiguous pairs.

### Emoji & Unicode Range

#### base100
```
Range:    U+1F3F7 to U+1F4F6 (256 emoji)
Example:  "Hi" → "🏿🐥"
Mode:     Byte range (1:1 mapping)
```
Direct byte-to-emoji encoding with zero overhead. Each byte maps to exactly one emoji. Inspired by [base💯](https://github.com/AdamNiederer/base100).

### Other Encodings

#### cards
```
Dictionary: 🂡🂢🂣...🃞 (52 playing cards)
Example:  "Hi" → "🃎🂾"
```
Encode data as playing cards.

#### dna
```
Dictionary: ACGT
Example:  "Hi" → "CAGACGGC"
```
Represent data as DNA sequences.

#### binary
```
Dictionary: 01
Example:  "Hi" → "100100001101001"
```
Pure binary representation.

## Usage Examples

```bash
# RFC standard
echo "Data" | base-d -a base64

# Bitcoin
echo "Address" | base-d -a base58

# Human-readable
echo "ID-12345" | base-d -a base32_crockford

# Emoji encoding (base100)
echo "Secret" | base-d -a base100

# Cards
echo "Secret" | base-d -a cards
```

## Comparison

```bash
Input: "Hello, World!"

base16:     48656C6C6F2C20576F726C6421
base32:     JBSWY3DPFQQFO33SNRSCC===
base64:     SGVsbG8sIFdvcmxkIQ==
base58:     72k1xXWG59fYdzSNoA
base62:     1wJfrzvdbtXUOlUjUf
ascii85:    bY4sj'`5!Ts/qKM9
base100:    🏿🐥🐬🐬🐯🐴🐀🐗🐯🐲🐬🐤🐁 (13 emoji, 1:1 byte mapping)
cards:      🂤🃉🂡🂾🂷🂸🂭🃓🃎🃉🂽🃕🂳🂻🃘🃃🃋🂮🂧🂶
```
