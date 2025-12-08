# UX Recommendations: base-d

## Summary

base-d delivers a surprisingly polished CLI experience for an encoding tool. Error messages are genuinely helpful with visual carets, contextual hints, and color support. The help text is well-organized and scannable. The main friction points are inconsistencies between documentation and implementation, a confusing default behavior (random compression when `-c` is used without an argument), and the loss of the documented `--encode` transcoding flag. The tool feels like it was built by someone who actually uses CLIs. A few rough edges remain, but the foundation is solid.

## Nielsen Heuristic Assessment

| Heuristic | Score (1-5) | Notes |
|-----------|-------------|-------|
| System status visibility | 4 | Good progress feedback via detect confidence, compression notes. Missing progress for large file ops. |
| Real world match | 5 | Familiar vocabulary: encode, decode, hash, compress. Dictionary names are intuitive. |
| User control | 4 | Has `--quiet`, `--no-color`, `--max-size`. Missing dry-run, undo not applicable. |
| Consistency | 3 | Doc vs CLI mismatch (`--encode` on decode). Mixed terminology (Radix vs base_conversion). |
| Error prevention | 4 | Size limits with override (`--force`). Destructive actions not really applicable. |
| Recognition over recall | 4 | Good `config list` command. Could show valid values inline in errors more often. |
| Flexibility | 5 | Novice can use defaults, experts get streaming, compression levels, custom dicts. |
| Minimalist design | 4 | Clean output, but detect prints to both stdout and stderr which can confuse piping. |
| Error recovery | 5 | Exceptional. Shows caret at error position, suggests fixes, provides valid characters. |
| Help/documentation | 4 | Good inline help, extensive docs. Some drift between them. |

## Friction Points

1. **Random compression default** - Using `-c` without specifying algorithm randomly selects one and prints a note. This is surprising behavior that violates least astonishment. A user expecting deterministic output gets random results.

2. **Missing `--encode` on decode** - README documents `base-d decode base64 --encode hex` for transcoding, but the flag doesn't exist. Users must pipe instead.

3. **File not found error is bare** - `No such file or directory (os error 2)` doesn't include the filename. User must guess which path was wrong.

4. **Detect outputs to both streams** - Decoded data goes to stdout, detection info goes to stderr. Fine for piping, but confusing when viewing output directly. The "Detected:" line interrupts the flow.

5. **Hash algorithm list is incomplete** - `config list hashes` shows 8 algorithms but docs mention 26. The shortened list omits crc32, sha3, blake2, etc.

## Confusion Points

1. **Mode terminology drift** - Config shows "Radix" mode, docs say "base_conversion", code uses both. Pick one.

2. **Dictionary vs algorithm** - `--compress` takes "algorithm" but there's no clear distinction in help text between compression algorithms and hash algorithms. Both are just "ALG".

3. **`config list` vs `config list dictionaries`** - Bare `config list` gives a human summary, adding `dictionaries` gives machine output. The difference isn't obvious.

4. **`-e` alias** - `encode` has alias `e`, suggesting `-e` might work for dictionary selection. It doesn't. (This is fine, just noting potential confusion.)

5. **Schema vs Fiche** - Two different structured data formats with overlapping purposes. When to use which?

## Delight Opportunities

1. **Did-you-mean suggestions** - Already implemented for typos in dictionary names. Extend to hash algorithms and compression methods.

2. **Shell completion** - Clap supports this. Would make discovering dictionaries and algorithms frictionless.

3. **Inline examples in help** - `base-d encode --help` could show 2-3 quick examples at the bottom.

4. **Progress indicator** - For `--stream` mode with large files, a progress bar or percentage would confirm it's working.

5. **Detect with auto-decompress** - If detection confidence is high and compression is detected, offer to decompress automatically.

## Findings

### Documentation-CLI Mismatch

- **Issue:** README documents `--encode` flag on decode command for transcoding, but it doesn't exist in CLI
- **Heuristic:** Consistency and standards
- **Impact:** Users trying documented examples get unexpected errors; trust in docs erodes
- **Suggestion:** Either implement `--encode` on decode, or update docs to show pipe-based transcoding
- **Priority:** High

### Bare OS Errors

- **Issue:** File-not-found error shows raw OS message without context: `No such file or directory (os error 2)`
- **Heuristic:** Help users recognize and recover from errors
- **Impact:** User must re-examine their command to find which path was wrong
- **Suggestion:** Wrap file errors with the attempted path: `error: file 'input.txt' not found (os error 2)`
- **Priority:** Medium

### Random Compression Behavior

- **Issue:** `--compress` without argument randomly selects algorithm and prints note to stderr
- **Heuristic:** Principle of least surprise
- **Impact:** Non-deterministic output breaks scripts, confuses users who expect errors
- **Suggestion:** Either require the algorithm argument or default to a sensible choice (gzip) without randomization
- **Priority:** Medium

### Incomplete Algorithm Lists

- **Issue:** `config list hashes` shows 8 algorithms; docs claim 26
- **Heuristic:** Recognition over recall
- **Impact:** Users may not discover available algorithms; docs feel unreliable
- **Suggestion:** Ensure `config list` shows all available options, or clarify why some are omitted
- **Priority:** Low

### Mixed Output Streams in Detect

- **Issue:** `detect` outputs decoded data to stdout and detection info to stderr, but this isn't documented
- **Heuristic:** Visibility of system status
- **Impact:** Users piping output may be confused by interleaved messages
- **Suggestion:** Add `--quiet` behavior that suppresses stderr info, or document the stream behavior
- **Priority:** Low

### Mode Terminology Inconsistency

- **Issue:** "Radix" in `config show`, "base_conversion" in TOML, "Mathematical" in docs
- **Heuristic:** Consistency and standards
- **Impact:** Users may think these are different modes
- **Suggestion:** Standardize on one term throughout CLI output, docs, and config
- **Priority:** Low

## What's Good

1. **Error messages are exceptional** - The invalid character error with caret positioning, context truncation, and valid character hints is genuinely best-in-class. This is how error messages should be done.

2. **Levenshtein suggestions** - Typo tolerance with "did you mean" suggestions shows attention to UX detail.

3. **Clean help organization** - Commands are logically grouped, flags are well-documented, aliases provide shortcuts for power users.

4. **Respects NO_COLOR** - Proper terminal detection and environment variable respect. Accessible by default.

5. **`config show` is informative** - Shows preview, mode, size, padding status, and common flag. Everything you need at a glance.

6. **Matrix mode has personality** - The "Wake up, Neo" intro, keyboard controls for dictionary switching, and `--superman` flag show care beyond pure utility. This kind of polish is rare.

7. **Exit codes are proper** - Exit 1 for errors, exit 2 for usage errors. Scripts can rely on these.

8. **Machine-readable options** - `--json` and plain output modes for config commands enable scripting.

9. **Sensible defaults** - 100MB size limit prevents accidental memory exhaustion, streamable for large files.

10. **Comprehensive documentation** - The docs directory is extensive with guides for every feature. The investment shows.

---

**Knock knock, Neo.**
