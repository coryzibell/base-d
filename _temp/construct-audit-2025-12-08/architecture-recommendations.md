# Architecture Recommendations

## Summary

base-d exhibits a well-structured layered architecture with clear separation between core primitives (dictionary, config), encoding algorithms, features (compression, hashing), and CLI. The schema subsystem demonstrates excellent trait-based abstraction for extensibility. The primary architectural concerns are: (1) the massive SIMD module (~15k LOC) with significant platform duplication, (2) the `fiche.rs` god module at 2440 lines, and (3) tight coupling between the CLI and library internals. The overall structure scales well but would benefit from extracting the schema encoding and SIMD implementations into separate crates.

## Architecture Style

**Primary Pattern:** Layered Architecture with Plugin-like Extensions

The codebase follows a clean layered approach:
- **Core Layer** (`core/`): Domain primitives - Dictionary, EncodingMode, DictionaryRegistry
- **Algorithm Layer** (`encoders/algorithms/`): Encoding implementations
- **Feature Layer** (`features/`): Optional capabilities - compression, hashing, detection
- **Acceleration Layer** (`simd/`): Platform-specific optimizations
- **Presentation Layer** (`cli/`): Command-line interface

The schema subsystem internally follows a **Pipeline/Ports-and-Adapters** pattern:
- Input ports: `InputParser` trait (JSON, Markdown implementations)
- Core: Intermediate Representation (IR)
- Output ports: `OutputSerializer` trait

This is appropriate for a library of this scope. The pattern is consistently applied.

## Coupling Analysis

| Module | Ca (Afferent) | Ce (Efferent) | Instability | Assessment |
|--------|---------------|---------------|-------------|------------|
| `core/dictionary` | High | Low | 0.15 | **Stable core** - correctly low instability |
| `core/config` | High | Low | 0.20 | **Stable core** - DictionaryRegistry used everywhere |
| `encoders/algorithms/radix` | Medium | Low | 0.25 | Stable algorithm module |
| `encoders/algorithms/chunked` | Medium | Low | 0.25 | Stable algorithm module |
| `encoders/algorithms/schema` | Medium | Medium | 0.50 | Balanced - could be more stable |
| `encoders/algorithms/schema/fiche` | Low | High | 0.85 | **Too unstable** for its size |
| `features/hashing` | Low | High | 0.80 | Appropriate for edge feature |
| `features/compression` | Low | High | 0.80 | Appropriate for edge feature |
| `simd/*` | Medium | Medium | 0.55 | Should be more stable given its size |
| `cli/handlers/*` | Low | High | 0.90 | Appropriate for presentation layer |
| `lib.rs` | Highest | High | 0.45 | **Facade concern** - too much re-export logic |

## Findings

### Structure Quality

#### Issue: God Module in Schema Subsystem
- **Location:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/fiche.rs` (2440 LOC)
- **SOLID Violation:** Single Responsibility Principle
- **Recommendation:** Extract into sub-modules:
  - `fiche/tokenizer.rs` - field/value tokenization
  - `fiche/parser.rs` - fiche string parsing
  - `fiche/serializer.rs` - IR to fiche output
  - `fiche/path_mode.rs` - path-based fiche format
  - `fiche/ascii.rs` - ASCII inline format
- **Priority:** Medium

#### Issue: SIMD Platform Duplication
- **Location:** `/home/kautau/work/personal/code/base-d/src/simd/` (~15,400 LOC)
- **SOLID Violation:** DRY (Don't Repeat Yourself)
- **Recommendation:** The `x86_64/specialized/` and `aarch64/specialized/` modules contain significant structural duplication. While the intrinsics differ, the algorithm structure is identical. Consider:
  1. Trait-based abstraction for SIMD operations
  2. Macro-based code generation for shared patterns
  3. Extract common logic into `simd/common/` with platform-specific leaf implementations
- **Priority:** Low (performance-critical code, refactoring risky)

#### Issue: Large LUT Modules
- **Location:** `/home/kautau/work/personal/code/base-d/src/simd/lut/base64.rs` (2510 LOC), `gapped.rs` (1685 LOC)
- **SOLID Violation:** Single Responsibility
- **Recommendation:** These modules handle both lookup table construction AND encoding/decoding logic. Separate:
  - Table generation (could be build-time/const)
  - Encoding/decoding algorithms
- **Priority:** Low

#### Issue: Generic SIMD Codec Size
- **Location:** `/home/kautau/work/personal/code/base-d/src/simd/generic/mod.rs` (2281 LOC)
- **SOLID Violation:** Single Responsibility
- **Recommendation:** Split into `generic/encode.rs`, `generic/decode.rs`, `generic/tables.rs`
- **Priority:** Low

### Dependency Analysis

#### Issue: lib.rs Re-export Complexity
- **Location:** `/home/kautau/work/personal/code/base-d/src/lib.rs:155-315`
- **SOLID Violation:** Interface Segregation
- **Recommendation:** The public API re-exports a large surface. Consider:
  1. Primary API in `lib.rs` root
  2. Extended schema API under `base_d::schema`
  3. Streaming API under `base_d::streaming`
  4. Move long doc comments to dedicated documentation
- **Priority:** Medium

#### Issue: CLI Depends on Library Internals
- **Location:** `/home/kautau/work/personal/code/base-d/src/cli/mod.rs:10` imports `base_d::DictionaryRegistry`
- **Assessment:** Appropriate - CLI as library consumer pattern
- **Note:** No violation, but the CLI could be a separate crate to enforce boundary

#### Issue: Schema Module Internal Dependencies
- **Location:** `encoders/algorithms/schema/`
- **Assessment:** Well-structured with trait abstractions
- **Note:** The `InputParser` and `OutputSerializer` traits enable extension without modification (Open/Closed compliant)

### Scalability Assessment

#### Natural Extension Points
1. **New Parsers:** Implement `InputParser` trait for YAML, CSV, TOML
2. **New Serializers:** Implement `OutputSerializer` trait
3. **New Dictionaries:** Add to `dictionaries.toml`
4. **New Compression:** Add to `CompressionAlgorithm` enum
5. **New Hash Algorithms:** Add to `HashAlgorithm` enum
6. **New SIMD Implementations:** Extend selection cascade in `simd/mod.rs`

#### Where New Features Should Go
| Feature Type | Location |
|--------------|----------|
| New encoding mode | `encoders/algorithms/` + update `EncodingMode` enum |
| New dictionary | `dictionaries.toml` (no code change) |
| New compression | `features/compression.rs` + update enum |
| New hash | `features/hashing.rs` + update enum |
| Custom input format | Implement `InputParser` in `schema/parsers/` |
| Custom output format | Implement `OutputSerializer` in `schema/serializers/` |
| New CLI command | `cli/args.rs` + `cli/handlers/` |
| SIMD for new platform | New module under `simd/` |

## Anti-Patterns Flagged

### God Module
- **Location:** `fiche.rs` (2440 LOC)
- **Impact:** Hard to test, modify, and understand
- **Severity:** Medium

### Potential Feature Envy
- **Location:** `cli/commands.rs` (595 LOC)
- **Assessment:** Legacy command handling that reaches into library internals
- **Note:** Marked for potential deprecation based on file size vs `handlers/` modules

### Deep Nesting (Borderline)
- **Location:** `encoders/algorithms/schema/parsers/` (4 levels)
- **Assessment:** At the threshold but justified by domain complexity
- **Recommendation:** Consider flattening `parsers/` to `schema/json_parser.rs`, `schema/markdown_parser.rs`

## Diagram Notes

The architecture diagram shows:
1. **Layered structure** with clear dependency direction (outer layers depend on inner)
2. **Schema pipeline** with three distinct phases: parse, binary, frame
3. **SIMD selection cascade** showing the decision tree for acceleration
4. The CLI properly sits outside the library boundary, depending only on public API

Key insight: The schema subsystem is essentially a separate product embedded within base-d. It could be extracted into `base-d-schema` crate if the project grows further.

## What's Good

### Excellent Trait Abstraction in Schema
The `InputParser` and `OutputSerializer` traits in `encoders/algorithms/schema/` demonstrate proper Open/Closed principle. Adding CSV support requires zero changes to existing code - just implement the trait.

```rust
// From types.rs - clean trait definitions
pub trait InputParser {
    type Error;
    fn parse(input: &str) -> Result<IntermediateRepresentation, Self::Error>;
}

pub trait OutputSerializer {
    type Error;
    fn serialize(ir: &IntermediateRepresentation, pretty: bool) -> Result<String, Self::Error>;
}
```

### Builder Pattern for Dictionary
`DictionaryBuilder` in `core/dictionary.rs` provides a clean fluent API while the deprecated `new()` methods maintain backwards compatibility with clear migration path.

### Well-Structured Error Types
`encoders/algorithms/errors.rs` provides rich error context with `DictionaryNotFoundError` including suggestion for closest match - good UX for library consumers.

### Clean Separation of CLI Handlers
Each command (`encode`, `decode`, `schema`, `fiche`, `hash`) has its own handler module. This follows Single Responsibility and makes adding new commands straightforward.

### Streaming Architecture
The `encoders/streaming/` module properly separates `StreamingEncoder`, `StreamingDecoder`, and `StreamingHasher` - each handles one concern.

### Feature Module Independence
`features/compression.rs`, `features/hashing.rs`, and `features/detection.rs` have minimal coupling to each other. They could be feature-gated individually.

### SIMD Automatic Selection
The cascade in `simd/mod.rs` tries specialized implementations first, then falls back to generic, then to LUT-based, then to scalar. This graceful degradation is well-designed.

### Prelude Pattern
`prelude.rs` provides ergonomic imports for common use cases without polluting the root namespace.

---

## Recommendations Summary

| Priority | Issue | Action |
|----------|-------|--------|
| **High** | None | Architecture is sound |
| **Medium** | fiche.rs god module | Extract to sub-modules |
| **Medium** | lib.rs re-export complexity | Organize into sub-modules |
| **Low** | SIMD platform duplication | Consider macro/trait abstraction |
| **Low** | Large LUT modules | Split table generation from algorithms |
| **Low** | schema/ deep nesting | Flatten parser modules |

## Implementation Order for Smith

If implementing changes based on these recommendations:

1. **First:** Extract `fiche.rs` into sub-modules (isolated change, high impact on maintainability)
2. **Second:** Reorganize `lib.rs` exports (improves API clarity)
3. **Third:** Consider schema crate extraction (if project scope expands)
4. **Defer:** SIMD refactoring (high risk, performance-critical)

Component ownership suggestion:
- `core/`, `encoders/algorithms/` - Core team
- `simd/` - Performance specialist
- `cli/` - CLI maintainer
- `features/` - Feature contributors
- `schema/` - Schema/serialization specialist
