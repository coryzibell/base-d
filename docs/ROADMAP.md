# Roadmap

## Phase 1: Core Functionality (v0.1.0) - COMPLETED ✓

- ✅ Implement base encoding/decoding algorithm
- ✅ Define Alphabet trait/struct
- ✅ Implement Playing Cards alphabet (52 characters)
  - Unicode range: 🂡-🃞 (U+1F0A1 to U+1F0DE)
- ✅ Basic library API
- ✅ Unit tests for encoding/decoding

## Phase 2: CLI Tool (v0.2.0) - COMPLETED ✓

- ✅ Command-line interface
  - ✅ Alphabet selection via `-a/--alphabet` flag
  - ✅ Decode mode via `-d/--decode` flag
  - ✅ List alphabets via `-l/--list` flag
- ✅ Input/output options (stdin, files)
- ✅ Error handling and user-friendly messages

## Phase 3: Additional Alphabets (v0.3.0) - COMPLETED ✓

- ✅ Custom alphabet support (TOML-based configuration)
- ✅ Additional built-in alphabets (33 total):
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

- [ ] Performance optimizations (Issue #14)
  - [ ] Profile encoding/decoding algorithms
  - [ ] Optimize hot paths
  - [ ] Add SIMD support where applicable
- [ ] Streaming encoding/decoding for large files (Issue #4)
  - [ ] Chunk-based processing
  - [ ] Reduce memory footprint
  - [ ] Support stdin/stdout streaming
- [ ] Enhanced alphabet validation and safety checks (Issue #5)
  - [ ] Duplicate character detection (already done)
  - [ ] Invalid Unicode handling
  - [ ] Size constraints validation
  - [ ] Character compatibility checks
- [ ] Configuration file support for custom alphabets (Issue #7)
  - [ ] Load from ~/.config/base-d/alphabets.toml
  - [ ] User-defined alphabets
  - [ ] Override built-in alphabets
- [ ] Web Assembly (WASM) support (Issue #6)
  - [ ] WASM compilation target
  - [ ] Browser compatibility
  - [ ] JavaScript bindings

## Phase 5: Polish (v1.0.0)

- [ ] Comprehensive documentation (Issue #2)
  - [ ] API documentation
  - [ ] Architecture overview
  - [ ] Contributing guidelines
  - [ ] Use case examples
- [ ] Benchmark suite (Issue #10)
  - [ ] Performance benchmarks across alphabets
  - [ ] Different data sizes
  - [ ] Encoding mode comparisons
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

- Visual alphabet representations (Issue #11)
- Interactive alphabet designer (Issue #8)
- Compression options (Issue #12)
- Checksum/error detection (Issue #13)
- Multiple output formats (Issue #15)

## Completed Milestones

- **2025-11-23**: Added ByteRange encoding mode and base100 alphabet (Issue #16)
- **2025-11-23**: Completed Phase 3 with 33 built-in alphabets
- **2025-11-23**: Added three encoding modes (mathematical, chunked, byte_range)
