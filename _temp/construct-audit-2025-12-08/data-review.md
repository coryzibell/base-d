# Data Review

## Summary

base-d is a CLI encoding tool with **no traditional database**. Data persistence consists of:
1. **Configuration files (TOML)** - dictionary definitions with multi-tier override system
2. **Fiche schema format** - in-memory structured data representation with binary packing
3. **Transient state** - HashMaps for tokenization, value deduplication, and lookup tables

The data layer is well-designed for its purpose: **ephemeral encoding/decoding operations**. Configuration is read-only at runtime. The fiche format is a sophisticated in-memory schema with built-in compression techniques (value dictionaries, field tokenization).

## Schema Overview

### Configuration Layer (TOML)
- **Structure**: `DictionaryRegistry` containing dictionaries, compression configs, settings
- **Storage**: Three-tier cascade (built-in → `~/.config/base-d/dictionaries.toml` → `./dictionaries.toml`)
- **Purpose**: Define encoding alphabets, modes, padding, compression defaults

### Fiche Format (In-Memory Schema)
- **Structure**: `IntermediateRepresentation` with header + row-major value array
- **Binary packing**: Varint encoding, null bitmaps, type tags, IEEE 754 floats
- **Tokenization**: Runic alphabet for field names, Egyptian hieroglyphs for repeated values
- **Purpose**: LLM-to-LLM wire protocol - binary-packed, display-safe, parser-inert

## Database Configuration

- **Type:** None (file-based config + in-memory operations)
- **Connection pooling:** N/A
- **Replication:** N/A

## Schema Assessment

| Component | Normalization | Indexes | Constraints | Issues |
|-----------|---------------|---------|-------------|--------|
| DictionaryRegistry | 1NF (flat HashMap) | HashMap keys | Serde validation | None - appropriate for config |
| DictionaryConfig | 3NF (atomic fields) | N/A | Manual validation in `effective_chars()` | Surrogate gap check correct |
| Fiche IR (header) | 3NF (fields normalized) | N/A | Row/field count validation | Well-structured |
| Fiche IR (values) | Denormalized (row-major array) | Calculated index | Type matching via `matches_type()` | Intentional denorm for perf |
| Token maps | N/A (ephemeral HashMap) | HashMap lookup | None | Appropriate for runtime |
| Value dictionaries | N/A (ephemeral HashMap) | HashMap lookup | Frequency threshold ≥ 2 | Good compression heuristic |

## Findings

### Configuration Management

- **Issue:** Configuration load path traversal vulnerability partially mitigated
- **Location:** `/home/kautau/work/personal/code/base-d/src/cli/config.rs:10-28` (`validate_config_path`)
- **Impact:** Security - prevents malicious path traversal in user-provided file paths
- **Recommendation:** Good security practice. Path canonicalization enforces `~/.config/base-d/` boundary.
- **Priority:** Low (already implemented correctly)

### Schema Validation

- **Issue:** Unicode range validation includes surrogate gap check
- **Location:** `/home/kautau/work/personal/code/base-d/src/core/config.rs:104-111`
- **Impact:** Data integrity - prevents invalid Unicode sequences
- **Recommendation:** Excellent validation. Correctly rejects ranges crossing U+D800..U+DFFF.
- **Priority:** Low (working as intended)

### Type Safety

- **Issue:** Runtime type matching instead of compile-time guarantees
- **Location:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/types.rs:309-323`
- **Impact:** Potential runtime type mismatches caught at IR construction
- **Recommendation:** Acceptable tradeoff for dynamic schema. `IntermediateRepresentation::new()` validates value count. Consider adding per-value type checks during construction.
- **Priority:** Medium

### Value Dictionary Compression

- **Issue:** Value dictionary uses frequency threshold of ≥ 2 occurrences
- **Location:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/fiche.rs:327-346`
- **Impact:** Performance - good heuristic for compression efficiency
- **Recommendation:** Well-designed. Sorts by frequency (most common first) to maximize token reuse. Hieroglyph alphabet (1072 chars) provides ample space.
- **Priority:** Low (optimal design)

### Null Bitmap Handling

- **Issue:** Null bitmap size validation present
- **Location:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/types.rs:256-266` (read), errors at `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/schema/types.rs:453-462`
- **Impact:** Data integrity - prevents bitmap size mismatches
- **Recommendation:** Proper error handling with `InvalidNullBitmap` error type.
- **Priority:** Low (correctly implemented)

### Configuration Merge Strategy

- **Issue:** Dictionary merge uses last-write-wins for duplicate keys
- **Location:** `/home/kautau/work/personal/code/base-d/src/core/config.rs:270-277`
- **Impact:** Behavior - local config overrides user config overrides built-in
- **Recommendation:** Correct semantics for configuration cascade. No warning on override (intentional design).
- **Priority:** Low (working as designed)

## N+1 Query Analysis

Not applicable - no database queries. All operations are in-memory transformations.

## Missing Indexes

Not applicable - uses HashMap for O(1) lookups where indexing is needed:
- `char_to_index` in Dictionary (decode lookups)
- `token_map` and `value_dict` in fiche parsing
- `dictionaries` HashMap in DictionaryRegistry

## Slow Query Candidates

Not applicable - no query layer. File I/O patterns:

| Operation | Location | Performance | Notes |
|-----------|----------|-------------|-------|
| Config load | `DictionaryRegistry::load_with_overrides()` | Sequential file reads (3 locations) | Acceptable - startup overhead only |
| TOML parse | `toml::from_str()` | Depends on file size | Built-in dictionaries.toml is ~18KB |
| Secret file read | `cli/config.rs:124` | Single `fs::read()` | After path validation |

## Migration Safety

Not applicable - no schema migrations. Config format is versioned implicitly via:
- Serde field defaults (`#[serde(default)]`)
- Optional fields for backward compatibility
- No version number in TOML (relies on field presence)

**Recommendation:** Consider adding explicit schema version to TOML for future extensibility:
```toml
[settings]
schema_version = 1
```

## Data Safety Assessment

| Concern | Status | Notes |
|---------|--------|-------|
| Encryption at rest | ✅ Pass | No sensitive data stored; xxHash secrets validated to `~/.config/base-d/` |
| PII protection | ✅ Pass | Tool processes user data transiently; no persistence |
| Backups | N/A | No database to back up; user configs in `~/.config/` |
| Audit logging | N/A | CLI tool - no audit requirements |
| Path traversal | ✅ Pass | `validate_config_path()` enforces canonical path checks |
| Input validation | ✅ Pass | Comprehensive Unicode validation, duplicate checks, mode constraints |

## Data Integrity

### Constraints Enforced

**DictionaryConfig (TOML validation):**
- Non-empty character sets (unless ByteRange mode)
- No duplicate characters (HashMap insertion check)
- Power-of-two requirement for Chunked mode
- Valid Unicode codepoint ranges
- Surrogate gap exclusion (U+D800..U+DFFF)
- Control character filtering

**IntermediateRepresentation:**
- Value count must equal `row_count × field_count`
- Null bitmap size must match value count (ceil(count/8) bytes)
- Type tags validated on deserialization (0-7 range)

**Fiche Format:**
- Field tokenization bounded by Runic alphabet (89 chars)
- Value tokenization bounded by Hieroglyphs (1072 chars)
- Token map dictionaries validated during parse

### Missing Constraints

- No runtime type validation during IR value assignment (relies on caller)
- No max value count limit (could cause OOM on malicious input)
- No max field count limit (unbounded memory allocation)

**Recommendation:** Add sanity limits for parsing untrusted fiche data:
```rust
const MAX_ROW_COUNT: usize = 1_000_000;
const MAX_FIELD_COUNT: usize = 10_000;
```

## Performance Patterns

### In-Memory Efficiency

| Pattern | Implementation | Assessment |
|---------|----------------|------------|
| Row-major layout | Flat `Vec<SchemaValue>` | ✅ Cache-friendly sequential access |
| Lookup tables | 256-element array for ASCII | ✅ O(1) decode for common chars |
| Value deduplication | Frequency-based dictionary | ✅ Reduces output size for repeated values |
| Token allocation | Most-frequent-first ordering | ✅ Minimizes token space usage |
| Null tracking | Bitmap (1 bit per value) | ✅ Compact null representation |

### HashMap Usage

**Appropriate use cases:**
- `DictionaryRegistry.dictionaries` - name → config lookup (startup only)
- `Dictionary.char_to_index` - decode char → index (decode hot path)
- `token_map` / `value_dict` - tokenization (serialization/parsing)

**No HashMap anti-patterns detected.** All usages are for legitimate O(1) lookups.

### Memory Allocation

**Good practices:**
- Pre-sized allocations: `String::with_capacity(length * 4)` for Unicode ranges
- Reuse: Single IR instance passed through pipeline
- No unnecessary cloning in hot paths

**Potential improvement:**
- Value dictionary builds full frequency map even when not needed (when `tokenize_values = false`)
- Could short-circuit early in `build_value_dictionary()` if not tokenizing

## Recommendations

### Immediate

1. **Add input sanity limits** (High)
   - Max row count / field count to prevent OOM attacks
   - Add to `IntermediateRepresentation::new()` validation

2. **Type validation during IR construction** (Medium)
   - Check value types match field types when adding to IR
   - Fail fast rather than during serialization

### Short-term

3. **Schema versioning** (Medium)
   - Add `schema_version` to TOML settings
   - Enable future backward-compatible migrations

4. **Optimize value dictionary** (Low)
   - Skip frequency counting when `tokenize_values = false`
   - Minimal perf impact (only during fiche serialization)

### Long-term

5. **Consider streaming fiche parser** (Low)
   - Current parser loads entire string into memory
   - For large datasets, streaming would reduce memory footprint
   - Trade-off: complexity vs use case (LLM wire protocol typically small)

6. **Formalize fiche spec version** (Low)
   - Currently at "spec 1.8+" based on comments
   - Add version byte to binary format for future extensibility

## What's Good

**Excellent design decisions:**

1. **Configuration cascade** - Three-tier override system (built-in → user → local) is intuitive and flexible

2. **Unicode validation** - Comprehensive checks for surrogate gaps, control chars, and range validity prevent subtle bugs

3. **Type safety via enums** - `FieldType` and `SchemaValue` enums provide exhaustive pattern matching and clear error messages

4. **Frequency-based compression** - Value dictionary uses occurrence counting (≥ 2) to compress efficiently without over-tokenizing

5. **Row-major layout** - Cache-friendly data structure for columnar operations

6. **Varint encoding** - Space-efficient integer representation in binary format

7. **Null bitmap** - Compact representation (1 bit per value) instead of sentinel values

8. **Path validation** - Security-conscious canonicalization prevents directory traversal

9. **Error context** - `SchemaError` variants include position and context for debugging

10. **No premature persistence** - Tool correctly avoids database overhead for ephemeral encoding operations

**Patterns worth maintaining:**
- HashMap for O(1) lookups where needed
- Builder pattern for Dictionary construction
- Explicit encoding modes instead of auto-detection magic
- Comprehensive unit tests for edge cases (surrogate gaps, range overflow, type mismatches)

---

**Data layer health: Excellent for a stateless CLI tool.**

The schema design is thoughtful, type-safe, and well-validated. No traditional database is appropriate here - configuration via TOML and in-memory transformations are the right architectural choices. The fiche format demonstrates sophisticated understanding of data compression, tokenization, and binary encoding.

Main improvement area: add sanity limits for untrusted input to prevent resource exhaustion.

---

Knock knock, Neo.
