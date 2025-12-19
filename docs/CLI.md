# CLI Reference

base-d's command line interface. Encode, decode, hash, compress — all in one tool.

## Commands at a glance

| Command | Alias | What it does |
|---------|-------|--------------|
| `encode` | `e` | Encode data using a dictionary |
| `decode` | `d` | Decode data from a dictionary |
| `detect` | — | Auto-detect dictionary and decode |
| `hash` | — | Compute hash of data |
| `schema` | — | Compact binary encoding (carrier98) |
| `fiche` | — | Model-readable structured format |
| `config` | — | List dictionaries, algorithms, hashes |
| `neo` | — | Matrix mode (the fun one) |

---

## encode

Encode data using any dictionary.

```bash
base-d encode <DICTIONARY> [FILE]
```

### Examples

```bash
# From stdin
echo "hello" | base-d encode base64
# aGVsbG8=

# From file
base-d encode base64 secret.txt

# Playing cards (the default vibe)
echo "secret" | base-d encode cards

# Ancient scripts
echo "pharaoh" | base-d encode hieroglyphics
echo "vikings" | base-d encode runes

# Emoji
echo "mood" | base-d encode emoji_faces

# Word-based encoding
echo "secret" | base-d encode bip39
# abandon absorb morning random...

echo "hello" | base-d encode pokemon
# bulbasaur charmander squirtle...

echo "data" | base-d encode nato
# alfa-bravo-charlie-delta

# Output to file
echo "data" | base-d encode base64 -o encoded.txt
```

### Options

| Flag | Description |
|------|-------------|
| `-c, --compress [ALG]` | Compress before encoding (gzip, zstd, brotli, lz4, snappy, lzma) |
| `--level <N>` | Compression level |
| `--hash <ALG>` | Also compute hash of input |
| `-s, --stream` | Streaming mode for large files (constant 4KB memory) |
| `-o, --output <FILE>` | Write to file instead of stdout |

### Compress + encode

```bash
# Compress with zstd, then encode as base64
base-d encode base64 --compress zstd < bigfile.json

# With compression level
base-d encode base64 --compress zstd --level 19 < bigfile.json

# Compress with gzip (default if no algorithm specified)
echo "data" | base-d encode base64 -c
```

### Streaming large files

```bash
# Process a 10GB file with constant memory
base-d encode base64 --stream < huge.bin > huge.b64
```

---

## decode

Decode data from a known dictionary.

```bash
base-d decode <DICTIONARY> [FILE]
```

### Examples

```bash
# From stdin
echo "aGVsbG8=" | base-d decode base64
# hello

# From file
base-d decode base64 encoded.txt

# With decompression
base-d decode base64 --decompress zstd < compressed.b64
```

### Options

| Flag | Description |
|------|-------------|
| `--decompress <ALG>` | Decompress after decoding |
| `--hash <ALG>` | Compute hash of decoded data |
| `-s, --stream` | Streaming mode for large files |
| `-o, --output <FILE>` | Write to file instead of stdout |

---

## detect

Don't know which dictionary was used? Let base-d figure it out.

```bash
base-d detect [FILE]
```

### Examples

```bash
# Auto-detect and decode
echo "aGVsbG8=" | base-d detect
# hello

# Show candidate dictionaries
echo "aGVsbG8=" | base-d detect --show-candidates 5
```

### How it works

1. Analyzes character set of input
2. Scores against all known dictionaries
3. Picks best match and decodes

Works best with longer inputs. Short strings may be ambiguous.

---

## hash

Compute hashes using 26 algorithms.

```bash
base-d hash <ALGORITHM> [FILE]
```

### Examples

```bash
# SHA-256
echo "hello" | base-d hash sha256
# 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824

# BLAKE3 (fast)
base-d hash blake3 bigfile.bin

# CRC32
base-d hash crc32 data.bin

# xxHash3 (very fast)
base-d hash xxh3 data.bin
```

### Available algorithms

**Cryptographic:** sha256, sha384, sha512, sha3-256, sha3-512, blake2b, blake2s, blake3, ascon, ascon-a

**Checksums:** crc32, crc32c, adler32

**Fast hashing:** xxh32, xxh64, xxh3, fnv1a-32, fnv1a-64

**Legacy:** md5, sha1 (not recommended for security)

### Encode the hash

```bash
# Output hash as base64 instead of hex
base-d hash sha256 --encode base64 < file.bin

# Output as emoji (why not)
base-d hash sha256 --encode emoji_faces < file.bin
```

---

## schema

Compact binary encoding for structured data. Preserves types, compresses well.

```bash
base-d schema [FILE]
base-d schema -d [FILE]  # decode
```

### Examples

```bash
# Encode JSON to compact binary
echo '{"name": "Neo", "age": 30}' | base-d schema
# (binary output)

# Decode back to JSON
base-d schema -d < encoded.bin

# Pretty-print decoded JSON
base-d schema -d --pretty < encoded.bin

# With compression
echo '{"data": [...]}' | base-d schema --compress zstd
```

[More on schema encoding →](SCHEMA.md)

---

## fiche

Model-readable structured format. Designed for LLM context windows.

```bash
base-d fiche [encode|decode] [OPTIONS] [INPUT]
```

### Examples

```bash
# Encode JSON
echo '{"user": "neo", "role": "admin"}' | base-d fiche encode
# ᚠuser᛬neo᛫ᚠrole᛬admin

# Decode back
echo 'ᚠuser᛬neo᛫ᚠrole᛬admin' | base-d fiche decode
# {"user": "neo", "role": "admin"}

# Pretty output
base-d fiche decode --pretty < encoded.fiche
```

### Modes

| Mode | Best for | Description |
|------|----------|-------------|
| `auto` | Most cases | Auto-detect best mode (default) |
| `none` | Debugging | No tokenization, fully human readable |
| `light` | Balance | Tokenize field names only (runic) |
| `full` | Compression | Tokenize fields + repeated values |
| `ascii` | JSON data | Inline CSV-like format, compact |
| `markdown` | Markdown input | Parse markdown documents |

**When to use which:**
- Start with `auto` — it picks based on your input structure
- Use `ascii` for JSON when you want maximum LLM readability
- Use `markdown` when your input is a markdown document, not JSON
- Use `none` when debugging to see exactly what's happening

```bash
# Explicit mode
echo '{"a": 1}' | base-d fiche encode --mode ascii
```

[More on fiche encoding →](SCHEMA.md)

---

## config

Query available dictionaries, algorithms, and hashes.

```bash
base-d config list [TYPE]
base-d config show <DICTIONARY>
```

### Examples

```bash
# List all dictionaries
base-d config list dictionaries

# List hash algorithms
base-d config list hashes

# List compression algorithms
base-d config list algorithms

# Show dictionary details
base-d config show base64

# JSON output (for scripting)
base-d config list dictionaries --json
```

---

## neo

Matrix mode. Because terminals should be fun.

```bash
base-d neo [OPTIONS]
```

### Examples

```bash
# Default Matrix rain
base-d neo

# Use a different dictionary
base-d neo --dictionary hieroglyphics

# Random dictionary (surprise me)
base-d neo --dejavu

# Cycle through all dictionaries
base-d neo --cycle

# Random dictionary switching with interval
base-d neo --random --interval 5s

# Maximum speed (remove 500ms delay)
base-d neo --superman
```

### Options

| Flag | Description |
|------|-------------|
| `--dictionary <DICT>` | Use specific dictionary (default: base256_matrix) |
| `--dejavu` | Use random dictionary |
| `--cycle` | Cycle through all dictionaries in order |
| `--random` | Random dictionary switching |
| `--interval <TIME>` | Switch interval: `5s`, `500ms`, or `line` |
| `--superman` | Remove 500ms delay, go full speed |

Press `Ctrl+C` to exit.

[More on neo mode →](NEO.md)

---

## Global options

These work with any command:

| Flag | Description |
|------|-------------|
| `-r, --raw` | Output raw binary (no encoding) |
| `-q, --quiet` | Suppress informational messages |
| `--no-color` | Disable colored output |
| `--max-size <N>` | Max input size in bytes (default: 100MB) |
| `--force` | Process files exceeding max-size |
| `-h, --help` | Show help |
| `-V, --version` | Show version |

---

## Common workflows

### Replace base64

```bash
# Encode
echo "data" | base-d e base64

# Decode
echo "ZGF0YQ==" | base-d d base64
```

### Replace sha256sum

```bash
# Hash a file
base-d hash sha256 file.bin

# Verify (compare output)
base-d hash sha256 file.bin | grep "expected_hash"
```

### Compress + encode + hash

```bash
# All in one pipeline
base-d encode base64 --compress zstd --hash sha256 < data.json
```

### Encode for different contexts

```bash
# URL-safe (no +, /, =)
echo "data" | base-d encode base64url

# Filesystem-safe
echo "filename" | base-d encode base58

# Copy-paste friendly (no ambiguous chars)
echo "token" | base-d encode base32
```

### Word-based encoding

```bash
# BIP-39 seed phrase style
echo "my secret key" | base-d encode bip39
# abandon absorb morning random throw...

# Fun encodings
echo "hello" | base-d encode pokemon
echo "hello" | base-d encode klingon
echo "message" | base-d encode nato

# Security-focused word lists
echo "data" | base-d encode diceware
echo "data" | base-d encode eff_long
```

### Process large files efficiently

```bash
# Streaming mode - constant memory usage
base-d encode base64 --stream < 10gb.bin > 10gb.b64
base-d decode base64 --stream < 10gb.b64 > 10gb.bin
```

### Auto-detect unknown encoding

```bash
# "What encoding is this?"
cat mystery.txt | base-d detect

# Show top candidates
cat mystery.txt | base-d detect --show-candidates 3
```

### Custom dictionaries

```bash
# List your custom dictionaries
base-d config list dictionaries

# Use a custom dictionary
echo "secret" | base-d encode my_custom_dict
```

[Create custom dictionaries →](CUSTOM_DICTIONARIES.md)

---

## Tips

### Aliases

Add to your shell rc:

```bash
alias b64='base-d encode base64'
alias b64d='base-d decode base64'
alias sha='base-d hash sha256'
```

### Piping

base-d reads stdin and writes stdout by default. Compose freely:

```bash
curl -s https://example.com/data.json \
  | base-d encode base64 --compress zstd \
  | base-d hash sha256 --encode base64
```

### Typo suggestions

Mistype a dictionary name? base-d will suggest the closest match:

```bash
$ base-d encode bas64
error: dictionary 'bas64' not found

hint: did you mean 'base64'?
```

Works for any dictionary name using fuzzy matching.

### Error handling

Exit codes:
- `0` — Success
- `1` — Error (check stderr for details)

```bash
base-d decode base64 < file.txt || echo "Decode failed"
```
