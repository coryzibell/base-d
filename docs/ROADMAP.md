# Roadmap

## Phase 1: Core Functionality (v0.1.0) - COMPLETED ✓

- ✅ Implement base encoding/decoding algorithm
- ✅ Define Dictionary trait/struct
- ✅ Implement Playing Cards dictionary (52 characters)
  - Unicode range: 🂡-🃞 (U+1F0A1 to U+1F0DE)
- ✅ Basic library API
- ✅ Unit tests for encoding/decoding

## Phase 2: CLI Tool (v0.2.0) - COMPLETED ✓

- ✅ Command-line interface
  - ✅ Dictionary selection via `-a/--dictionary` flag
  - ✅ Decode mode via `-d/--decode` flag
  - ✅ List dictionaries via `-l/--list` flag
- ✅ Input/output options (stdin, files)
- ✅ Error handling and user-friendly messages

## Phase 3: Additional Dictionaries (v0.3.0) - COMPLETED ✓

- ✅ Custom dictionary support (TOML-based configuration)
- ✅ Additional built-in dictionaries (33 total):
  - ✅ RFC 4648 Standards (5): base16, base32, base32hex, base64, base64url
  - ✅ Bitcoin & Blockchain (2): base58, base58flickr
  - ✅ High-Density Encodings (4): base62, base85, ascii85, z85
  - ✅ Human-Oriented (2): base32_crockford, base32_zbase
  - ✅ Ancient Scripts (3): hieroglyphs, cuneiform, runic
  - ✅ Game Pieces (4): cards, domino, mahjong, chess
  - ✅ Esoteric Symbols (5): alchemy, zodiac, weather, music, arrows
  - ✅ Emoji (3): emoji_faces, emoji_animals, base100
  - ✅ Other (5): dna, binary, hex, base64_math, hex_math
- ✅ Three encoding modes:
  - ✅ Mathematical base conversion
  - ✅ RFC 4648 chunked mode
  - ✅ ByteRange mode (direct byte-to-character mapping)

## Phase 4: Advanced Features (v0.4.0) - IN PROGRESS

- [x] Performance optimizations (Issue #14)
  - [x] Profile encoding/decoding algorithms
  - [x] Optimize hot paths (chunked, byte_range, mathematical)
  - [x] Add comprehensive benchmark suite
  - [x] Implement fast lookup tables for ASCII dictionaries
  - [x] Optimize memory allocation (pre-allocation)
  - [x] Chunk-based processing for CPU cache optimization
  - [ ] Add SIMD support where applicable (future enhancement)
- [ ] Streaming encoding/decoding for large files (Issue #4)
  - [x] Chunk-based processing (already implemented)
  - [x] Reduce memory footprint (4KB chunks)
  - [x] Support stdin/stdout streaming
  - [ ] Add progress reporting for CLI
- [ ] Enhanced dictionary validation and safety checks (Issue #5)
  - [x] Duplicate character detection (already done)
  - [x] Invalid Unicode handling (already done)
  - [x] Size constraints validation (already done)
  - [x] Character compatibility checks (already done)
- [ ] Configuration file support for custom dictionaries (Issue #7)
  - [x] Load from built-in dictionaries.toml
  - [ ] Load from ~/.config/base-d/dictionaries.toml
  - [ ] User-defined dictionaries
  - [ ] Override built-in dictionaries
- [ ] Web Assembly (WASM) support (Issue #6)
  - [ ] WASM compilation target
  - [ ] Browser compatibility
  - [ ] JavaScript bindings

## Phase 5: Polish (v1.0.0)

- [ ] Comprehensive documentation (Issue #2)
  - [x] Performance documentation (PERFORMANCE.md)
  - [ ] API documentation
  - [ ] Architecture overview
  - [ ] Contributing guidelines
  - [ ] Use case examples
- [x] Benchmark suite (Issue #10)
  - [x] Performance benchmarks across dictionaries
  - [x] Different data sizes
  - [x] Encoding mode comparisons
  - [x] Criterion.rs integration with HTML reports
- [ ] Examples and tutorials (Issue #1)
  - [ ] Common use cases
  - [ ] Integration examples
  - [ ] Best practices
- [ ] API stabilization (Issue #9)
  - [ ] Review all public interfaces
  - [ ] Consistent naming
  - [ ] Deprecation warnings
  - [ ] Breaking changes documentation
- [ ] Release v1.0.0 (Issue #3)

## Future Considerations

- Visual dictionary representations (Issue #11)
- Interactive dictionary designer (Issue #8)
- Compression options (Issue #12)
- Checksum/error detection (Issue #13)
- Multiple output formats (Issue #15)
- `--verbose` flag for dejavu mode to show which dictionary was selected (currently silent for puzzle effect)

## Completed Milestones

- **2025-11-23**: Implemented performance optimizations with benchmark suite
  - Added Criterion.rs benchmarks for base64, base32, base100, hex
  - Optimized chunked encoding/decoding (370 MiB/s encode, 220 MiB/s decode)
  - Fast lookup tables for ASCII dictionaries (5x faster character decoding)
  - Memory allocation optimizations (pre-allocation, chunk processing)
  - Created PERFORMANCE.md documentation
- **2025-11-23**: Added ByteRange encoding mode and base100 dictionary (Issue #16)
- **2025-11-23**: Completed Phase 3 with numerous built-in dictionaries
- **2025-11-23**: Added three encoding modes (mathematical, chunked, byte_range)
