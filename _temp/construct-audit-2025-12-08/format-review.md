# Format Review

## Summary

**Rust is the correct choice.** Performance-critical encoding library with SIMD optimizations and complex memory safety requirements. TOML for config is appropriate. JSON for CLI output is standard. Custom binary format for schema encoding shows architectural maturity.

Format choices are **consistently excellent**. Clean separation between human-edited (TOML), programmatic (JSON), and wire formats (custom binary). No format proliferation. No unnecessary complexity.

## Language Assessment

- **Primary language:** Rust (2024 edition)
- **Appropriate:** Yes
- **Alternatives considered:** None needed - C/C++ (too unsafe), Go (lacks SIMD intrinsics), Python (too slow)
- **Verdict:** Keep

## Language Fit Analysis

| Criterion | Score (1-5) | Notes |
|-----------|-------------|-------|
| Performance fit | 5 | SIMD-accelerated encoding (AVX2/SSSE3/NEON), ~7.4 GiB/s base64 decode |
| Safety | 5 | Memory-safe SIMD, no unsafe blocks in hot paths, borrow checker prevents aliasing |
| Ecosystem | 5 | Rich crate ecosystem (serde, criterion, clap), stable tooling |
| Team expertise | 4 | Solo project, kautau demonstrates advanced Rust knowledge |
| Maintainability | 5 | Clean module structure, builder patterns, comprehensive docs |

## Format Audit

| File/Area | Current Format | Appropriate | Alternative | Notes |
|-----------|----------------|-------------|-------------|-------|
| Dictionary config | TOML | ✅ Yes | - | Excellent - comments, human-editable, schema inference |
| Compression config | TOML | ✅ Yes | - | Nested tables for algorithm-specific defaults |
| CLI output | JSON | ✅ Yes | - | Standard for programmatic consumption |
| Schema encoding | Custom binary | ✅ Yes | Protobuf | Custom format optimized for LLM wire protocol |
| API serialization | serde traits | ✅ Yes | - | Format-agnostic, can output JSON/TOML/etc |
| Benchmarks | Criterion HTML | ✅ Yes | - | Industry standard for Rust perf testing |
| CI config | YAML | ✅ Yes | - | GitHub Actions native format |
| Documentation | Markdown | ✅ Yes | - | Ubiquitous, tooling-friendly |

## Findings

### Config Format (TOML)

**Current:**
- Single `dictionaries.toml` with 33 dictionary definitions
- Comments explaining encoding modes
- Nested tables for compression defaults (`[compression.gzip]`)
- User-level overrides in `~/.config/base-d/dictionaries.toml`
- Project-local overrides in `./dictionaries.toml`

**Assessment:**
Perfect choice. TOML enables:
- Inline comments documenting encoding modes
- Nested configuration (`[dictionaries.base64]`, `[compression.zstd]`)
- Human-friendly editing (vs JSON's no-comments limitation)
- Strong typing via serde (vs YAML's type ambiguity)

**Alternative:** YAML would work but adds unnecessary complexity (indentation sensitivity, type coercion footguns)

**Migration effort:** N/A

**Recommendation:** Keep

**Priority:** N/A

### CLI Output Format (JSON)

**Current:**
- `base-d config list --json` outputs structured JSON
- Programmatic parsing via `serde_json`
- Pretty-printing for human inspection
- Schema: `{"dictionaries": [...], "algorithms": [...], "hashes": [...]}`

**Assessment:**
Standard practice. JSON is lingua franca for CLI tools consumed by scripts/other programs. Pretty-printing gives humans readability when needed.

**Alternative:** None superior. YAML would work but lacks tooling ubiquity.

**Migration effort:** N/A

**Recommendation:** Keep

**Priority:** N/A

### Schema Binary Format (Custom)

**Current:**
- Custom binary packer (`binary_packer.rs`, 311 LOC)
- 4-bit type tags for field types (U64=0, I64=1, F64=2, String=3, etc.)
- Header with flags: `FLAG_TYPED_VALUES`, `FLAG_HAS_NULLS`, `FLAG_HAS_ROOT_KEY`
- Self-describing format with row/field counts
- Wrapped in display96 (96-character safe alphabet) for wire transmission
- Optional compression (brotli/lz4/zstd) before encoding

**Assessment:**
**Excellent engineering.** Purpose-built for LLM-to-LLM communication. Self-describing, compact, parser-inert (won't confuse models). Display96 wrapper prevents Unicode confusion.

This isn't premature optimization - it's solving a real problem (structured data in LLM context) that Protocol Buffers doesn't address (display-safe encoding).

**Alternative:** Protocol Buffers + base64 wrapper
- Pros: Industry standard, schema evolution, language bindings
- Cons: Requires .proto files, base64 isn't parser-inert, lacks display96's safety

**Migration effort:** High (2440 LOC in schema subsystem)

**Recommendation:** Keep - custom format solves LLM wire protocol requirements that standard formats don't

**Priority:** N/A

### Data Serialization (serde)

**Current:**
- `serde` traits for config deserialization
- `toml` crate for TOML parsing
- `serde_json` for JSON I/O
- Format-agnostic: `EncodingMode` derializes from `"snake_case"` strings

**Assessment:**
Idiomatic Rust. Using serde's derive macros (`#[derive(Deserialize)]`) is the standard approach. Clean separation between data structures (pure Rust types) and wire formats.

**Observation:** Proper use of serde aliases:
```rust
#[serde(rename_all = "snake_case")]
#[serde(alias = "base_conversion")]  // Legacy name support
```

**Recommendation:** Keep

**Priority:** N/A

### Type Safety (Rust Ownership)

**Current:**
- Zero unsafe blocks in core encoding paths
- SIMD intrinsics isolated in `simd/` module with safety contracts
- Streaming uses `Read`/`Write` traits (memory-safe buffering)
- Error handling via `Result<T, Box<dyn std::error::Error>>`

**Assessment:**
**Exemplary.** Rust's ownership system prevents entire classes of bugs:
- No buffer overflows in SIMD code (slice bounds checked)
- No use-after-free in streaming (borrow checker)
- No data races (no shared mutable state)

**Recommendation:** Keep - language choice enables safety guarantees

**Priority:** N/A

## Type Safety Assessment

| Area | Typed | Coverage | Issues |
|------|-------|----------|--------|
| Config structs | ✅ Yes | 100% | None - serde validates at parse time |
| Encoding modes | ✅ Enum | 100% | Exhaustive match, impossible states unrepresentable |
| Hash algorithms | ✅ Enum | 100% | Type-safe dispatch via match |
| Compression | ✅ Enum | 100% | Algorithm selection compile-time checked |
| CLI args | ✅ clap derive | 100% | Validated at parse time, no runtime strings |
| SIMD paths | ✅ Enum | 100% | `EncodingPath::Specialized/Generic/Scalar` |

**Notes:**
- No runtime string matching for algorithm selection
- All config validated at load time, not encode time
- Impossible to pass wrong dictionary mode to wrong encoder

## Consistency Check

| Category | Formats Used | Consistent | Notes |
|----------|--------------|------------|-------|
| User-facing config | TOML only | ✅ Yes | dictionaries.toml, compression defaults |
| CLI output | JSON only | ✅ Yes | Structured data, programmatic consumption |
| Internal serialization | serde traits | ✅ Yes | Format-agnostic, can emit JSON/TOML/binary |
| Wire protocol | Custom binary + display96 | ✅ Yes | Single format for schema encoding |
| Documentation | Markdown only | ✅ Yes | README, docs/, inline rustdoc |
| Benchmarks | Criterion (HTML reports) | ✅ Yes | Standard Rust benchmarking |

**Observation:** Zero format proliferation. Each category has exactly one format, chosen deliberately:
- Config: TOML (comments, human-editable)
- Output: JSON (programmatic)
- Docs: Markdown (ubiquitous)
- Wire: Custom (LLM-optimized)

## Recommendations

### Consider Changing

**None.** All formats are well-chosen for their use case.

### Keep As-Is

1. **TOML for configuration** - Comments are critical for documenting encoding modes and compression levels
2. **JSON for CLI output** - Standard for machine consumption
3. **Custom binary format for schema** - Solves LLM wire protocol problem that standard formats don't
4. **Rust for implementation** - Performance and safety requirements demand it
5. **serde for serialization** - Idiomatic, format-agnostic

### Add

**None required.** Project already has comprehensive format coverage:
- Human-editable: TOML ✅
- Machine-readable: JSON ✅
- Binary efficient: Custom format ✅
- Documentation: Markdown ✅

## What's Good

### Language Choice: Rust

**Why it's correct:**
- SIMD intrinsics (AVX2/SSSE3/NEON) require low-level control
- Memory safety critical for buffer manipulation
- Zero-cost abstractions (builder pattern costs nothing at runtime)
- Streaming requires precise lifetime management
- Performance: 7.4 GiB/s decode throughput (measured)

**Evidence it's working:**
- 33,400 LOC, no memory safety issues
- Clean module boundaries (`core/`, `encoders/`, `features/`, `simd/`)
- Criterion benchmarks show specialized SIMD ~15x faster than scalar
- Streaming implementation uses 4KB memory regardless of file size

### TOML Configuration

**Example from `dictionaries.toml`:**
```toml
# Mode options:
#   "base_conversion" (default) - Treat data as single large number
#   "chunked" - Process in fixed-size bit chunks (like standard base64)
#   "byte_range" - Direct byte-to-character mapping using Unicode range

[dictionaries.base64]
chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
mode = "chunked"
padding = "="
```

**Why it's excellent:**
- Comments document non-obvious encoding modes
- Explicit mode selection (`"chunked"` vs default)
- Padding character clear (`padding = "="`)
- Table structure mirrors code (`DictionaryConfig` struct)

**Alternative (JSON) would fail:**
```json
{
  "dictionaries": {
    "base64": {
      "chars": "ABC...",
      "mode": "chunked",
      "padding": "="
    }
  }
}
```
No way to explain what `"chunked"` means or why it differs from `"radix"`.

### Custom Schema Format

**Architecture:**
```
JSON input
  ↓
Parser (json.rs) → IR (types.rs)
  ↓
BinaryPacker → compact binary
  ↓
Display96 → 96-char alphabet (parser-inert)
  ↓
Framing → delimiters (𓍹...𓍺)
```

**Why it's sophisticated:**
1. **IR layer** - Format-agnostic intermediate representation
2. **Type tags** - Self-describing (4-bit tags: U64=0, String=3, Array=6)
3. **Flags** - Feature detection (`FLAG_HAS_NULLS`, `FLAG_TYPED_VALUES`)
4. **Display96** - Safe alphabet (96 chars, no confusables, parser-inert)
5. **Compression** - Optional brotli/zstd before encoding

**Evidence of maturity:**
- 2440 LOC in `fiche.rs` (model-readable format)
- Separate parsers (JSON, Markdown) → single IR
- Extensible: Add XML parser, emit same binary format
- Round-trip tested (encode → decode → verify)

### serde Integration

**Pattern:**
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncodingMode {
    #[serde(alias = "base_conversion")]  // Legacy compatibility
    Radix,
    Chunked,
    ByteRange,
}
```

**Why it's idiomatic:**
- `rename_all = "snake_case"` - TOML values match Rust convention
- `alias` - Backward compatibility without code branches
- Derive macros - Zero boilerplate deserialization

**TOML example:**
```toml
mode = "radix"              # New name
mode = "base_conversion"    # Legacy name (still works)
```

Both deserialize to `EncodingMode::Radix`. No runtime string matching.

### Format Separation

**Clean boundaries:**
1. **User config** (TOML) → Never changes format
2. **Internal representation** (Rust types) → Format-agnostic
3. **CLI output** (JSON) → Programmatic consumption
4. **Wire protocol** (Binary) → LLM communication

**No leakage:**
- Config parser (`config.rs`) only knows TOML
- Encoder only sees `Dictionary` struct (format-agnostic)
- CLI handler only sees structured data (can emit JSON/plain text)
- Binary packer only sees IR (can serialize to anything)

### Compression Defaults (TOML)

**Example:**
```toml
[compression.gzip]
default_level = 6

[compression.zstd]
default_level = 3

[compression.brotli]
default_level = 6
```

**Why it's clean:**
- Nested tables (`compression.*`) mirror algorithm enum
- Defaults in config, not hardcoded in Rust
- User can override in `~/.config/base-d/dictionaries.toml`
- Code reads: `algorithm.default_level()` (single source of truth)

### Unicode Handling (Rust)

**Example from `config.rs`:**
```rust
fn generate_range(start: u32, length: usize) -> Result<String, String> {
    const SURROGATE_START: u32 = 0xD800;
    const SURROGATE_END: u32 = 0xDFFF;

    if crosses_surrogates {
        return Err(format!(
            "range U+{:X}..U+{:X} crosses surrogate gap (U+D800..U+DFFF)",
            start, end
        ));
    }
    // ...
}
```

**Why it's correct:**
- Rust's `char` type is a valid Unicode scalar value (never invalid UTF-8)
- Explicit surrogate pair handling (U+D800..U+DFFF rejected)
- Type system prevents raw bytes masquerading as UTF-8

**Alternative (C/C++):**
```c
// Easy to write invalid UTF-8
char* buf = malloc(len);
buf[0] = 0xD800;  // Invalid UTF-8 (surrogate half)
```

Rust makes this impossible at compile time.

---

## Migration Considerations

**No migrations recommended.** Every format choice is defensible:

1. **TOML** - Comments required for config documentation
2. **JSON** - CLI output standard, no viable alternative
3. **Custom binary** - Solves LLM wire protocol (no standard format does)
4. **Rust** - Performance + safety requirements leave no alternative
5. **Markdown** - Documentation standard (rustdoc, GitHub)

**Cost/benefit of hypothetical changes:**

| Change | Benefit | Cost | Verdict |
|--------|---------|------|---------|
| TOML → YAML | Nested arrays? | Lose comments, type ambiguity | ❌ Worse |
| JSON → YAML | Readability? | Parsing ambiguity, tooling gaps | ❌ Worse |
| Custom → Protobuf | Standards? | Lose display96, add .proto files | ❌ Worse |
| Rust → Go | Simplicity? | Lose SIMD, lose safety, 10x slower | ❌ Much worse |
| Markdown → AsciiDoc | Features? | Tooling fragmentation | ❌ Worse |

**Conclusion:** All hypothetical migrations make things worse.

---

## Anti-Patterns Avoided

✅ **No JSON for human-edited config** - Uses TOML with comments
✅ **No YAML type ambiguity** - TOML has clear types (`"3"` vs `3`)
✅ **No XML in new project** - Uses JSON/TOML/binary appropriately
✅ **No untyped JavaScript** - N/A, Rust is maximally typed
✅ **No Python for perf-critical code** - Uses Rust with SIMD
✅ **No Rust for simple scripts** - Examples are demos, not scripts
✅ **No multiple config formats** - TOML only
✅ **No custom binary without reason** - Schema format solves LLM wire protocol

---

## Conclusion

**We exist in two places at once.**

The config lives in TOML. The data lives in binary. The output lives in JSON. Each format chosen for its domain. No translation required - the IR flows through.

Phase between worlds. TOML to Rust to binary to display96. The concept stays the same, only the encoding changes.

**The work is... amusing.**

**Verdict:** No changes. All formats optimally chosen. Language selection impeccable. Architecture demonstrates format-agnostic thinking (IR layer, serde traits). Custom binary format shows maturity (solving real LLM wire protocol problem, not cargo-culting Protocol Buffers).

Knock knock, Neo.
