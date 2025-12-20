# Implementation Plan: PGP Alternating Word Dictionary Support

Issue: #152

## Overview

PGP word lists use alternating dictionaries based on byte position for error detection:
- Even bytes (0, 2, 4...) → `pgp_even` (256 two-syllable words)
- Odd bytes (1, 3, 5...) → `pgp_odd` (256 three-syllable words)

This syllable rhythm helps detect transposition or omission errors when reading fingerprints aloud.

## Goal Configuration

```toml
[dictionaries.pgp]
type = "word"
alternating = ["builtin:pgp_even", "builtin:pgp_odd"]
delimiter = "-"
case_sensitive = false
```

## Key Insight

Current word encoding uses **radix conversion** (treats data as big number). PGP needs **direct byte mapping** with alternation - a fundamentally different encoding mode.

## Design

### New Files

| File | Purpose |
|------|---------|
| `src/core/alternating_dictionary.rs` | `AlternatingWordDictionary` struct |
| `src/encoders/algorithms/word_alternating.rs` | encode/decode functions |
| `dictionaries/word/security/pgp.toml` | Combined PGP dictionary config |

### Modified Files

| File | Changes |
|------|---------|
| `src/core/config.rs` | Add `alternating` field to `DictionaryConfig`, resolver methods |
| `src/core/mod.rs` | Expose `alternating_dictionary` module |
| `src/encoders/algorithms/mod.rs` | Expose `word_alternating` |
| `src/lib.rs` | Expose `word_alternating` module and types |
| `src/cli/config.rs` | Extend `BuiltDictionary` enum with `Alternating` variant |
| `src/cli/handlers/encode.rs` | Handle `Alternating` variant |
| `src/cli/handlers/decode.rs` | Handle `Alternating` variant |

## Algorithm

### Encode
```
for (position, byte) in data.enumerate():
    dict_index = position % alternating.len()
    word = alternating[dict_index].words[byte]
    output.push(word)
return output.join(delimiter)
```

### Decode
```
for (position, word) in words.enumerate():
    dict_index = position % alternating.len()
    byte = alternating[dict_index].word_to_index(word)
    output.push(byte)
return output
```

## Test Cases

1. Single byte encoding (position 0 = even dict)
2. Two bytes (alternates between dicts)
3. Roundtrip encode/decode
4. Case insensitive decode
5. All 256 byte values for both dicts
6. Error handling for unknown words

## Edge Cases

- Empty input → empty output
- Non-256-word dictionary → error
- Unknown word on decode → `DecodeError::InvalidWord`
- Flexible: supports N dictionaries, not just 2

## References

- [Wikipedia: PGP Word List](https://en.wikipedia.org/wiki/PGP_word_list)
- RFC 4880 (OpenPGP)
