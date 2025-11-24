# Compression Support

base-d includes built-in support for multiple compression algorithms that can be applied before encoding or after decoding. This allows you to significantly reduce the size of encoded data, especially for text or repetitive content.

## Supported Algorithms

| Algorithm | Flag | Default Level | Best For |
|-----------|------|---------------|----------|
| **gzip** | `--compress gzip` | 6 | General purpose, wide compatibility |
| **zstd** | `--compress zstd` | 3 | Fast compression, excellent ratio |
| **brotli** | `--compress brotli` | 6 | Web content, best compression ratio |
| **lz4** | `--compress lz4` | 0 | Fastest speed, lower ratio |
| **snappy** | `--compress snappy` | 0 | Very fast, moderate ratio, Google |
| **lzma** | `--compress lzma` | 6 | Maximum compression, slower speed |

## Basic Usage

### Compress and Encode

```bash
# Compress with gzip, encode with base64 (default)
echo "Hello, World!" | base-d --compress gzip

# Compress with zstd, encode with custom dictionary
echo "Data" | base-d --compress zstd -e base85

# Specify compression level (1-9, algorithm dependent)
echo "Text" | base-d --compress brotli --level 11 -e base64
```

### Decode and Decompress

```bash
# Decode from base64, decompress with gzip
echo "H4sIAAAAAAAA//NIzcnJ11EIzy/KSVHkAgCEnui0DgAAAA==" | base-d -d base64 --decompress gzip

# Full pipeline: decode → decompress → display
cat compressed.txt | base-d -d base64 --decompress zstd
```

### Raw Compressed Output

```bash
# Output raw compressed binary (no encoding)
echo "Data to compress" | base-d --compress gzip --raw > output.gz

# Read raw compressed input (no decoding)
cat output.gz | base-d --decompress gzip
```

## Configuration

Compression defaults are configured in `dictionaries.toml`:

```toml
[settings]
default_dictionary = "base64"  # Used when compressing without --encode

[compression.gzip]
default_level = 6

[compression.zstd]
default_level = 3

[compression.brotli]
default_level = 6

[compression.lz4]
default_level = 0

[compression.snappy]
default_level = 0

[compression.lzma]
default_level = 6
```

Override these in:
- User config: `~/.config/base-d/dictionaries.toml`
- Project config: `./dictionaries.toml`

## Compression Levels

Each algorithm has different level ranges and characteristics:

### gzip
- **Range**: 0-9
- **Level 1**: Fastest, lower ratio
- **Level 6**: Default, balanced
- **Level 9**: Best compression, slower

### zstd
- **Range**: 1-22
- **Level 1**: Very fast
- **Level 3**: Default, good balance
- **Level 19-22**: Ultra compression (slow)

### brotli
- **Range**: 0-11
- **Level 0**: Fastest
- **Level 6**: Default
- **Level 11**: Maximum compression

### lz4
- **Range**: N/A (single mode)
- **Level**: Ignored (always uses fast mode)
- **Characteristics**: Extremely fast decompression

### snappy
- **Range**: N/A (single mode)
- **Level**: Ignored (no compression levels)
- **Characteristics**: Very fast, designed by Google for speed

### lzma
- **Range**: 0-9
- **Level 0**: Fastest, lower ratio
- **Level 6**: Default, excellent compression
- **Level 9**: Maximum compression, very slow

## Pipeline Examples

### Simple Compress → Encode

```bash
# Compress file with gzip, encode with base64
base-d --compress gzip -e base64 large_file.txt > compressed.b64

# Decode and decompress
base-d -d base64 --decompress gzip compressed.b64 > restored.txt
```

### Transcode with Compression

```bash
# Decode base64 → compress with zstd → encode with base85
echo "SGVsbG8sIFdvcmxkIQ==" | base-d -d base64 --compress zstd -e base85
```

### Maximum Compression

```bash
# Use brotli level 11 for maximum compression
cat document.txt | base-d --compress brotli --level 11 -e base64 > tiny.txt
```

### Fast Compression

```bash
# Use lz4 for speed-critical applications
echo "Fast compression" | base-d --compress lz4 -e base64

# Use snappy for very fast compression
echo "Fast compression" | base-d --compress snappy -e base64
```

### Maximum Compression

```bash
# Use lzma level 9 for ultimate compression
cat document.txt | base-d --compress lzma --level 9 -e base64 > ultra-tiny.txt
```

## Performance Characteristics

### Compression Speed (Relative)
1. **snappy**: ~600 MB/s (fastest)
2. **lz4**: ~500 MB/s 
3. **zstd level 3**: ~300 MB/s
4. **gzip level 6**: ~80 MB/s
5. **brotli level 6**: ~30 MB/s
6. **lzma level 6**: ~10 MB/s (slowest)

### Decompression Speed (Relative)
1. **lz4**: ~2000 MB/s (fastest)
2. **zstd**: ~600 MB/s
3. **gzip**: ~250 MB/s
4. **brotli**: ~200 MB/s (slowest)

### Compression Ratio (Text, Relative)
1. **brotli level 11**: Best (~25% smaller than gzip)
2. **zstd level 19**: Excellent
3. **gzip level 9**: Good
4. **lz4**: Fair (~40% larger than gzip)

## Use Cases

### Web Content (brotli)
```bash
# Compress HTML/CSS/JS for web transmission
base-d --compress brotli --level 11 -e base64 index.html
```

### Log Files (zstd)
```bash
# Balance speed and ratio for log archival
base-d --compress zstd --level 9 -e base64 app.log > archived.log.txt
```

### Real-time Data (lz4)
```bash
# Minimal latency for streaming applications
cat sensor_data.bin | base-d --compress lz4 -e base64
```

### Cold Storage (gzip)
```bash
# Universal compatibility for long-term archival
base-d --compress gzip --level 9 -e base64 archive.tar
```

## Limitations

1. **Streaming Mode**: Not yet supported with compression
   ```bash
   # This will error
   base-d --stream --compress gzip -e base64 large.bin
   ```

2. **Memory Usage**: Compression loads entire file into memory (use streaming mode for large files when implemented)

3. **Algorithm Detection**: When decompressing, you must specify the correct algorithm
   ```bash
   # Must match compression algorithm
   base-d -d base64 --decompress gzip  # not --decompress zstd
   ```

## Library Usage

Compression is also available as a library:

```rust
use base_d::{compress, decompress, CompressionAlgorithm};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = b"Hello, World!";
    
    // Compress
    let compressed = compress(data, CompressionAlgorithm::Gzip, 6)?;
    
    // Decompress
    let decompressed = decompress(&compressed, CompressionAlgorithm::Gzip)?;
    
    assert_eq!(data, &decompressed[..]);
    Ok(())
}
```

## Comparison Examples

### Without Compression
```bash
$ echo "This is repeated text. This is repeated text." | base-d -e base64
VGhpcyBpcyByZXBlYXRlZCB0ZXh0LiBUaGlzIGlzIHJlcGVhdGVkIHRleHQuCg==
# 64 characters
```

### With Compression
```bash
$ echo "This is repeated text. This is repeated text." | base-d --compress gzip -e base64
H4sIAAAAAAAA/8tIzcnJVyjPL8pJUQhJLcpLzE1VKM8v0lEIy88vySzJzM9TSMxLUQIA1hIOdSwAAAA=
# 84 characters (overhead for small data)
```

### Large File (Better Ratio)
```bash
$ base-d -e base64 large_file.txt | wc -c
1048576  # 1 MB encoded

$ base-d --compress gzip -e base64 large_file.txt | wc -c
131072   # 128 KB compressed+encoded (87.5% reduction)
```

## Future Enhancements

Planned improvements (see ROADMAP.md):
- Streaming mode support for compression
- Additional algorithms (snappy, lzma)
- Parallel compression for multi-core systems
- Automatic algorithm selection based on content type
