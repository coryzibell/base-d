# Streaming Encoding/Decoding

base-d supports streaming mode for processing large files without loading them entirely into memory. This is particularly useful when working with files larger than available RAM.

## Overview

Streaming mode processes data in 4KB chunks, making it memory-efficient for large files while maintaining the same encoding/decoding guarantees.

## Usage

Add the `--stream` (or `-s`) flag to enable streaming mode:

```bash
# Stream encode a large file
base-d --stream -a base64 large_file.bin > encoded.txt

# Stream decode
base-d --stream -a base64 -d encoded.txt > decoded.bin

# Works with stdin/stdout
cat large_file.bin | base-d --stream -a base64 | base-d --stream -a base64 -d > output.bin
```

## Encoding Mode Support

### Chunked Mode (Full Streaming Support)

RFC 4648 encodings (base64, base32, base16) fully support streaming:

```bash
# Encode 1GB file with streaming
base-d --stream -a base64 huge_file.dat > encoded.txt
```

Benefits:
- Constant memory usage (~4KB buffer)
- No temporary files
- Progress can be monitored with `pv`

### Byte Range Mode (Full Streaming Support)

Direct byte-to-character mapping also streams perfectly:

```bash
# base100 with streaming
base-d --stream -a base100 data.bin > emoji.txt
```

Benefits:
- 1:1 byte mapping
- Minimal overhead
- Perfect for emoji encodings

### Mathematical Base Conversion (Limited Support)

Mathematical mode treats data as a single large number, so it requires the entire input:

```bash
# This will still load the entire file into memory
base-d --stream -a cards large_file.bin > output.txt
```

While the `--stream` flag is accepted, mathematical mode (used by cards, dna, etc.) will read the entire input before encoding.

## Performance Comparison

### Memory Usage

| Mode | Standard | Streaming |
|------|----------|-----------|
| base64 (10MB file) | 10MB | ~4KB |
| base64 (1GB file) | 1GB | ~4KB |
| base100 (10MB file) | 10MB | ~4KB |
| cards (any size) | Full file size | Full file size* |

*Mathematical mode always requires full input

### Speed

Streaming mode has minimal performance overhead:

```bash
# Benchmark encoding 100MB file
$ time base-d -a base64 100mb.bin > /dev/null
real    0m0.45s

$ time base-d --stream -a base64 100mb.bin > /dev/null
real    0m0.47s
```

The ~4% overhead is acceptable for the memory savings.

## When to Use Streaming

### Use Streaming When:

1. **Large files** - Files larger than available RAM
2. **Pipeline processing** - Data flows through multiple tools
3. **Memory constraints** - Running on low-memory systems
4. **Chunked/ByteRange modes** - Using base64, base32, base100, etc.

### Don't Use Streaming When:

1. **Small files** - Files under 1MB (overhead not worth it)
2. **Mathematical modes** - cards, dna, emoji_faces (no benefit)
3. **Random access needed** - Need to seek within encoded data

## Examples

### Example 1: Process Large Log File

```bash
# Encode a 5GB log file
base-d --stream -a base64 access.log > access.b64

# Decode back
base-d --stream -a base64 -d access.b64 > access.log
```

### Example 2: Pipeline with Other Tools

```bash
# Compress, encode, and upload
tar czf - /data | base-d --stream -a base64 | aws s3 cp - s3://bucket/backup.b64
```

### Example 3: Monitor Progress

```bash
# Show progress while encoding
pv huge_file.bin | base-d --stream -a base64 > encoded.txt
```

### Example 4: Split Large Encoded File

```bash
# Encode and split into 100MB chunks
base-d --stream -a base64 large.bin | split -b 100M - encoded_part_
```

## Technical Details

### Chunk Size

Default chunk size is 4KB (4096 bytes), which balances:
- Memory efficiency
- I/O performance
- CPU cache usage

For chunked mode, chunks are aligned to encoding group boundaries to avoid padding issues.

### Buffer Management

- **Encoding**: Reads 4KB, encodes, writes immediately
- **Decoding**: Reads 4KB, accumulates complete character groups, decodes, writes

### Error Handling

Streaming mode provides the same error detection as standard mode:

```bash
$ echo "invalid!@#" | base-d --stream -a base64 -d
Error: InvalidCharacter('!')
```

Errors are detected as soon as invalid data is encountered, not after reading the entire input.

## Limitations

1. **Mathematical mode limitation**: No memory savings for base_conversion mode
2. **No random access**: Streaming is forward-only
3. **Progress reporting**: Standard mode can show progress percentage, streaming cannot

## API Usage

For library users, streaming is available programmatically:

```rust
use base_d::{AlphabetsConfig, Alphabet, StreamingEncoder, StreamingDecoder};
use std::fs::File;

// Encode
let config = AlphabetsConfig::load_default()?;
let alphabet_config = config.get_alphabet("base64").unwrap();
let chars: Vec<char> = alphabet_config.chars.chars().collect();
let alphabet = Alphabet::new_with_mode(
    chars,
    alphabet_config.mode.clone(),
    alphabet_config.padding.as_ref().and_then(|s| s.chars().next())
)?;

let mut input = File::open("large_file.bin")?;
let mut output = File::create("encoded.txt")?;

let mut encoder = StreamingEncoder::new(&alphabet, output);
encoder.encode(&mut input)?;
```

## See Also

- [Encoding Modes](ENCODING_MODES.md) - Understanding which modes support streaming
- [Performance Tips](../README.md#performance) - Optimization recommendations
- Issue #4 - Original feature request
