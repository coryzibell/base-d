# Compression Support Implementation

## Summary

Added comprehensive compression support to base-d, allowing users to compress data before encoding or decompress after decoding. This feature significantly reduces encoded output size for text and repetitive data.

## What Was Added

### 1. Four Compression Algorithms
- **gzip** - General purpose, wide compatibility (default level: 6)
- **zstd** - Fast with excellent ratio (default level: 3)
- **brotli** - Best compression ratio (default level: 6)
- **lz4** - Fastest speed (level: 0)

### 2. New CLI Flags
- `--compress <ALGORITHM>` - Compress before encoding
- `--decompress <ALGORITHM>` - Decompress after decoding
- `--level <N>` - Set compression level
- `--raw` - Output raw binary (no encoding)

### 3. Configuration Support
Added to `dictionaries.toml`:
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
```

### 4. New Dependencies
- `flate2` - gzip compression
- `zstd` - Zstandard compression
- `brotli` - Brotli compression
- `lz4` - LZ4 compression

### 5. Library API
Public exports in `lib.rs`:
- `CompressionAlgorithm` enum
- `compress()` function
- `decompress()` function
- `CompressionConfig` struct
- `Settings` struct

### 6. Processing Pipeline
Data flows through stages:
1. **Decode** (if `--decode` specified)
2. **Decompress** (if `--decompress` specified)
3. **Compress** (if `--compress` specified)
4. **Encode** (if `--encode` specified, or default if compressed)

## Usage Examples

### Basic Compression
```bash
# Compress with gzip, encode with base64 (default)
echo "Hello, World!" | base-d --compress gzip
```

### Custom Level
```bash
# Maximum compression with brotli
echo "Data" | base-d --compress brotli --level 11 -e base64
```

### Decompress
```bash
# Decode and decompress
echo "H4sIAAAAAAAA/..." | base-d -d base64 --decompress gzip
```

### Raw Output
```bash
# Output raw compressed binary
echo "Data" | base-d --compress gzip --raw > output.gz
```

### Mix Algorithms
```bash
# Any algorithm with any dictionary
echo "Test" | base-d --compress zstd -e emoji_faces
```

## Files Modified

1. **Cargo.toml** - Added compression dependencies
2. **src/compression.rs** - New module with compression logic
3. **src/lib.rs** - Export compression API
4. **src/core/config.rs** - Added `CompressionConfig` and `Settings`
5. **src/main.rs** - Added CLI flags and pipeline logic
6. **dictionaries.toml** - Added compression config and settings
7. **README.md** - Added compression examples
8. **docs/COMPRESSION.md** - Comprehensive documentation

## Testing

All tests pass:
- 48 unit tests (including 4 new compression roundtrip tests)
- 7 doc tests
- 6 integration tests for compression features

## Performance Impact

- No impact when compression is not used
- Compression adds processing time but significantly reduces output size
- Example: Repetitive text compressed with gzip can be 80%+ smaller

## Backward Compatibility

✓ Fully backward compatible - no breaking changes
- Existing functionality unchanged
- New flags are optional
- Default behavior preserved

## Future Enhancements

Potential improvements (add to ROADMAP.md):
- Streaming mode support for compression
- Additional algorithms (snappy, lzma)
- Parallel compression for large files
- Auto-detection of compressed data format
