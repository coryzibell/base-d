# Roadmap

## Phase 1: Core Functionality (v0.1.0)

- [ ] Implement base encoding/decoding algorithm
- [ ] Define Alphabet trait/struct
- [ ] Implement Playing Cards alphabet (52 characters)
  - Unicode range: 🂡-🃞 (U+1F0A1 to U+1F0DE)
- [ ] Basic library API
- [ ] Unit tests for encoding/decoding

## Phase 2: CLI Tool (v0.2.0) - COMPLETED ✓

- ✅ Command-line interface
  - ✅ Alphabet selection via `-a/--alphabet` flag
  - ✅ Decode mode via `-d/--decode` flag
  - ✅ List alphabets via `-l/--list` flag
- ✅ Input/output options (stdin, files)
- ✅ Error handling and user-friendly messages

## Phase 3: Additional Alphabets (v0.3.0)

- [ ] Custom alphabet support (user-provided character sets)
- [ ] Additional built-in alphabets:
  - [ ] Emoji set
  - [ ] Zodiac symbols
  - [ ] Chess pieces
  - [ ] Mahjong tiles
  - [ ] Domino tiles

## Phase 4: Advanced Features (v0.4.0)

- [ ] Performance optimizations
- [ ] Streaming encoding/decoding for large files
- [ ] Alphabet validation and safety checks
- [ ] Configuration file support for custom alphabets
- [ ] Web Assembly (WASM) support

## Phase 5: Polish (v1.0.0)

- [ ] Comprehensive documentation
- [ ] Benchmark suite
- [ ] Examples and tutorials
- [ ] API stabilization
- [ ] Release v1.0.0

## Future Considerations

- Visual alphabet representations
- Interactive alphabet designer
- Compression options
- Checksum/error detection
- Multiple output formats (hex, binary, etc.)
