# Accessibility

## Summary

base-d demonstrates strong CLI accessibility fundamentals with proper NO_COLOR support, terminal capability detection, and keyboard navigation. The tool respects accessibility standards for command-line applications. Key strengths include dual color/no-color output paths, machine-readable JSON output options, and comprehensive keyboard controls in interactive mode.

Areas for improvement include: incomplete NO_COLOR flag integration, lack of progress indicators for screen readers, no documented accessibility features, and missing internationalization framework.

**Overall Assessment:** Good foundation with clear accessibility-conscious design patterns. Implementation is 70% complete - core infrastructure exists but needs refinement and documentation.

## WCAG Compliance Assessment

| Level | Status | Gaps |
|-------|--------|------|
| A | Partial | Screen reader announcements for streaming operations; keyboard trap potential in Matrix mode |
| AA | Partial | High contrast mode detection; progress indicators; comprehensive documentation |
| AAA | Not Assessed | Not a typical target for CLI tools |

**Note:** WCAG is primarily designed for web content. This assessment adapts WCAG principles (POUR) to CLI accessibility best practices.

## Testing Performed

| Method | Completed | Notes |
|--------|-----------|-------|
| Code review | ✓ | Reviewed color handling, keyboard nav, output formatting |
| NO_COLOR compliance | ✓ | Tested environment variable and --no-color flag |
| Terminal detection | ✓ | Verified IsTerminal checks for stderr |
| Screen reader compatibility | Partial | No testing with actual screen readers (NVDA/VoiceOver) |
| Keyboard navigation | ✓ | Reviewed Matrix mode controls |
| JSON output | ✓ | Verified machine-readable output options |

## Findings

### Visual Accessibility - Color Usage

**Good:**
- `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/errors.rs:163` - Respects `NO_COLOR` environment variable
- `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/errors.rs:169` - Uses `IsTerminal` to detect terminal capability
- Conditional ANSI codes in Matrix mode based on `no_color` flag
- Color used for enhancement only, not essential information

**Issues:**

#### Issue 1: Incomplete NO_COLOR Flag Integration
- **WCAG Criterion:** 1.4.1 Use of Color (Level A)
- **Level:** A
- **Impact:** Users with `--no-color` flag may still see colored output in error messages
- **Location:** `/home/kautau/work/personal/code/base-d/src/cli/global.rs:16` defines `no_color` flag, but `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/errors.rs:163` only checks environment variable
- **Details:** The `should_use_color()` function checks `NO_COLOR` env var and terminal detection, but doesn't receive the `--no-color` flag from global args. Error formatting happens in library code that doesn't have access to CLI flags.
- **Recommendation:**
  - Option 1: Set `NO_COLOR=1` environment variable when `--no-color` flag is passed
  - Option 2: Add thread-local or global state for color preference
  - Option 3: Pass color preference through all error paths (breaking change)
- **Priority:** Medium

#### Issue 2: Matrix Mode Color Dependency
- **WCAG Criterion:** 1.4.1 Use of Color (Level A)
- **Level:** A
- **Impact:** Matrix mode uses color as primary visual effect; graceful degradation exists but experience is degraded
- **Location:** `/home/kautau/work/personal/code/base-d/src/cli/commands.rs:135-138`
- **Details:** Matrix mode provides `--no-color` support with plain text fallback, but the mode is inherently visual. Without color, it's just scrolling text.
- **Recommendation:** This is acceptable - Matrix mode is explicitly a visual effect. The tool provides graceful degradation. Consider documenting that Matrix mode is best experienced with color support.
- **Priority:** Low (documentation only)

### Keyboard Navigation

**Good:**
- `/home/kautau/work/personal/code/base-d/src/cli/commands.rs:307-364` - Comprehensive keyboard controls in Matrix mode
- Arrow keys (Left/Right) to switch dictionaries
- Space to pause
- Escape to exit
- Ctrl+C to exit
- No mouse required for any functionality

**Issues:**

#### Issue 3: Keyboard Navigation Not Documented
- **WCAG Criterion:** 3.2.1 On Focus (Level A) / 2.1.1 Keyboard (Level A)
- **Level:** A
- **Impact:** Users may not discover keyboard controls in Matrix mode
- **Location:** Matrix mode (`neo` command) - controls work but aren't shown on screen
- **Details:** Controls exist (Esc, Space, Left/Right) but aren't visible to users. No help text or documentation of keyboard shortcuts.
- **Recommendation:**
  - Show initial help overlay: "Press ? for help, Esc to exit"
  - Add `?` key to show/hide control help
  - Document controls in `--help` output
  - Consider showing controls when quiet mode is off
- **Priority:** High

#### Issue 4: Potential Keyboard Trap
- **WCAG Criterion:** 2.1.2 No Keyboard Trap (Level A)
- **Level:** A
- **Impact:** Users might not know how to exit Matrix mode
- **Location:** `/home/kautau/work/personal/code/base-d/src/cli/commands.rs:323` (Escape handler exists)
- **Details:** Escape and Ctrl+C work to exit, but this isn't immediately obvious. Terminal raw mode is enabled, changing normal terminal behavior.
- **Recommendation:** Show "Press Esc to exit" message initially or when `--quiet` is not set
- **Priority:** High

### Screen Reader Support

**Issues:**

#### Issue 5: No Screen Reader Announcements for Progress
- **WCAG Criterion:** 4.1.3 Status Messages (Level AA - WCAG 2.1)
- **Level:** AA
- **Impact:** Screen reader users don't get feedback during long operations
- **Location:** Streaming operations, encoding/decoding large files
- **Details:** No progress indicators or status messages during operations. Silent operation except for final output.
- **Recommendation:**
  - Add optional `--verbose` flag for screen-reader-friendly progress
  - Emit periodic status to stderr: "Processed 1MB... 2MB..."
  - Use `\r` for visual progress bars, `\n` for screen reader updates
  - Detect if stderr is a TTY - if not, use line-based updates
- **Priority:** Medium

#### Issue 6: Matrix Mode Not Accessible to Screen Readers
- **WCAG Criterion:** 1.1.1 Non-text Content (Level A)
- **Level:** A (with exemption for decorative content)
- **Impact:** Screen reader users cannot experience Matrix mode
- **Location:** Entire `neo` command
- **Details:** Matrix mode is a purely visual effect. Screen readers will read random encoded characters, which is meaningless.
- **Recommendation:**
  - This is acceptable - Matrix mode is decorative/artistic
  - Document that Matrix mode is a visual effect not designed for screen readers
  - Consider adding `--describe` mode that narrates what's happening: "Streaming base64 encoded random data... switching to hieroglyphics..."
- **Priority:** Low (documentation) / Medium (audio description mode)

### CLI Accessibility

| Feature | Status | Notes |
|---------|--------|-------|
| NO_COLOR env support | ✓ Good | Properly implemented in error formatting |
| --no-color flag | ⚠ Partial | Flag exists but doesn't reach library error code |
| Plain text mode | ✓ Good | All output modes work without color |
| Machine-parseable output | ✓ Good | JSON output via `--json` flag (config command) |
| Terminal capability detection | ✓ Good | Uses IsTerminal for stderr |
| Progress indicators | ✗ Missing | No progress feedback for long operations |
| Streaming output | ✓ Good | Line-based output works with screen readers |

#### Issue 7: Inconsistent Machine-Readable Output
- **WCAG Criterion:** 4.1.2 Name, Role, Value (Level A - adapted for CLI)
- **Level:** AA
- **Impact:** Assistive tools and scripts may have difficulty parsing some outputs
- **Location:** Various handler files
- **Details:**
  - Config command has `--json` flag for structured output
  - Detect command shows candidates but no JSON output option
  - Hash command outputs raw hex (good) or encoded, but no structured format with metadata
  - Schema command outputs JSON by default (good)
- **Recommendation:**
  - Add `--json` flag globally or to detect/hash commands
  - Structured output should include metadata: `{"algorithm": "base64", "data": "SGVsbG8=", "detected_confidence": 0.95}`
  - Exit codes should be documented and consistent
- **Priority:** Medium

### Motor Accessibility

**Assessment:** CLI tools have different motor accessibility considerations than GUIs.

**Good:**
- No mouse required
- Tab completion likely works (via shell, not application-provided)
- Single keypress controls in Matrix mode (Space, Esc, arrows)

**Issues:**

#### Issue 8: No Command Abbreviations or Aliases
- **WCAG Criterion:** 2.1.1 Keyboard (Level A) - adapted: minimize keystrokes
- **Level:** AAA (enhancement)
- **Impact:** Users with motor difficulties benefit from shorter commands
- **Location:** `/home/kautau/work/personal/code/base-d/src/cli/mod.rs:28,32` - Only `encode` and `decode` have visible aliases
- **Details:** Commands like `encode` and `decode` have short aliases (`e`, `d`), but others don't
- **Recommendation:**
  - Add visible aliases to other commands: `h` for hash, `s` for schema, `c` for config, `det` for detect
  - Document all aliases in help text
  - Consider allowing `b64`, `b32` as dictionary aliases
- **Priority:** Low

### Cognitive Accessibility

**Good:**
- Clear command structure (verb + dictionary pattern)
- Helpful error messages with suggestions (fuzzy matching for dictionary names)
- Consistent CLI patterns
- Self-documenting with `--help`

**Issues:**

#### Issue 9: Error Messages Could Be Clearer
- **WCAG Criterion:** 3.3.1 Error Identification (Level A), 3.3.3 Error Suggestion (Level AA)
- **Level:** AA
- **Impact:** Users may struggle to understand what went wrong
- **Location:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/errors.rs:202-220`
- **Details:**
  - Good: Dictionary not found error includes suggestions
  - Could improve: Some errors are technical ("Invalid dictionary: ...")
  - File size errors are clear (good)
- **Recommendation:**
  - Review all error messages for clarity
  - Follow pattern: "What went wrong" → "Why" → "How to fix"
  - Example: Instead of "Invalid dictionary", say "Dictionary 'xyz' could not be built: character set must be unique"
- **Priority:** Low

#### Issue 10: No Accessibility Documentation
- **WCAG Criterion:** 3.3.5 Help (Level AAA)
- **Level:** AA (documentation is important for CLI tools)
- **Impact:** Users don't know about accessibility features
- **Location:** README.md, documentation
- **Details:** No mention of:
  - NO_COLOR support
  - --no-color flag
  - JSON output for parsing
  - Keyboard controls in Matrix mode
  - Screen reader compatibility
- **Recommendation:** Add "Accessibility" section to README:
  ```markdown
  ## Accessibility

  base-d follows CLI accessibility best practices:

  - **Color-free output**: Set `NO_COLOR=1` environment variable or use `--no-color` flag
  - **Screen reader friendly**: Outputs to stdout/stderr appropriately
  - **Machine-readable output**: Use `--json` flag for structured data (config command)
  - **Keyboard navigation**: All features accessible via keyboard
  - **Matrix mode controls**: Esc (exit), Space (pause), Left/Right arrows (switch dictionary)

  For issues or accessibility requests, please open an issue.
  ```
- **Priority:** High

### Internationalization (i18n)

| Feature | Status | Notes |
|---------|--------|-------|
| Externalized strings | ✗ No | All strings are hardcoded in Rust code |
| RTL support | N/A | CLI tools typically follow terminal text direction |
| Date/number formatting | ✗ No | No date/number output requiring localization |
| Translation workflow | ✗ No | No i18n framework in place |
| Text expandability | ✓ Yes | Terminal wraps text naturally |
| Cultural assumptions | ✓ Good | Minimal cultural assumptions; Unicode-aware |

#### Issue 11: No Internationalization Support
- **WCAG Criterion:** 3.1.1 Language of Page (Level A) - adapted for CLI
- **Level:** Enhancement (not typically required for CLI tools)
- **Impact:** Non-English speakers must use English commands and error messages
- **Location:** Entire codebase
- **Details:**
  - All strings are hardcoded in English
  - No i18n framework (gettext, fluent, etc.)
  - Error messages are English-only
  - This is typical for CLI tools, but limits accessibility
- **Recommendation:**
  - For now: Ensure error messages are clear, simple English
  - Future: Consider i18n framework if user base becomes international
  - Priority: Ensure technical terms (base64, encode, etc.) remain in English as they're universal
- **Priority:** Low (not required for most CLI tools)

## Keyboard Navigation Audit

| Feature | Keyboard Accessible | Focus Visible | Notes |
|---------|---------------------|---------------|-------|
| All commands | ✓ Yes | N/A (CLI) | No mouse required anywhere |
| Matrix mode - Exit | ✓ Yes | N/A | Esc and Ctrl+C both work |
| Matrix mode - Pause | ✓ Yes | N/A | Space toggles pause |
| Matrix mode - Switch dictionary | ✓ Yes | N/A | Left/Right arrow keys |
| Matrix mode - Skip intro | ✓ Yes | N/A | Esc/Space/Enter during intro |
| Help access | ✓ Yes | N/A | --help flag on all commands |

**No keyboard traps detected** (once users know Esc exits Matrix mode).

## Color Contrast Audit

| Element | Ratio | Required | Pass | Notes |
|---------|-------|----------|------|-------|
| Matrix mode green | Unknown | 4.5:1 | Unknown | ANSI green `\x1b[32m` - actual ratio depends on terminal theme |
| Error messages red | Unknown | 4.5:1 | Unknown | ANSI red `\x1b[1;31m` - depends on terminal |
| General output | N/A | N/A | N/A | Uses terminal default colors |

**Note:** Terminal applications rely on user's terminal color scheme. Cannot guarantee contrast ratios. Mitigation: All colored output has plain-text alternative via NO_COLOR.

**Recommendation:** Document that users should configure terminal color schemes with sufficient contrast.

## Screen Reader Testing

**Status:** Not tested with actual screen readers (NVDA, JAWS, VoiceOver).

**Expected behavior:**
- ✓ Standard commands (encode, decode, hash) should work well - output to stdout, errors to stderr
- ✓ Piped input/output works with screen readers
- ⚠ Matrix mode will read random characters - not useful for screen reader users
- ⚠ Long operations are silent - no progress feedback
- ⚠ Keyboard shortcuts in Matrix mode are undiscovered without visual cues

**Recommendations for screen reader testing:**
1. Test with NVDA (Windows) or VoiceOver (macOS)
2. Verify error messages are announced properly
3. Test `neo` command - confirm Esc/Ctrl+C announcement
4. Test streaming operations with large files - verify completion is announced

## CLI Accessibility Details

### NO_COLOR Support

**Implementation:** `/home/kautau/work/personal/code/base-d/src/encoders/algorithms/errors.rs:162-170`

```rust
fn should_use_color() -> bool {
    // Respect NO_COLOR environment variable
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    // Check if stderr is a terminal
    use std::io::IsTerminal;
    std::io::stderr().is_terminal()
}
```

**Status:** ✓ Excellent - Follows NO_COLOR spec (https://no-color.org/)
- Checks environment variable
- Falls back to terminal detection
- Applies to error formatting

**Gap:** `--no-color` flag doesn't reach this function (see Issue 1)

### Plain Text Mode

**Status:** ✓ Good
- All functionality works without color
- Color is decorative only, never informational
- Matrix mode provides no-color fallback

### Machine-Parseable Output

**Implemented:**
- `/home/kautau/work/personal/code/base-d/src/cli/handlers/config.rs:41-53` - JSON output for config command
- Hash command outputs hex (parseable)
- Schema command outputs JSON
- Raw binary output via `--raw` flag

**Gaps:**
- Detect command lacks JSON output (Issue 7)
- No unified structured output format

## Recommendations

### Critical (Level A violations)

1. **Document keyboard controls in Matrix mode** (Issue 3)
   - Add help overlay or initial message
   - Document in `--help` output
   - **Effort:** Low (1-2 hours)
   - **Files:** `/home/kautau/work/personal/code/base-d/src/cli/commands.rs`, `/home/kautau/work/personal/code/base-d/src/cli/args.rs`

2. **Show exit instructions to prevent keyboard trap** (Issue 4)
   - Display "Press Esc to exit" when not in quiet mode
   - **Effort:** Low (30 minutes)
   - **File:** `/home/kautau/work/personal/code/base-d/src/cli/commands.rs:212-218`

3. **Add accessibility documentation** (Issue 10)
   - Add "Accessibility" section to README
   - Document NO_COLOR, keyboard controls, screen reader compatibility
   - **Effort:** Low (1 hour)
   - **File:** `/home/kautau/work/personal/code/base-d/README.md`

### Important (Level AA violations)

4. **Integrate --no-color flag with library error code** (Issue 1)
   - Set `NO_COLOR=1` environment variable when `--no-color` is passed
   - Most practical solution without architectural changes
   - **Effort:** Low (1 hour)
   - **Files:** `/home/kautau/work/personal/code/base-d/src/cli/mod.rs`, `/home/kautau/work/personal/code/base-d/src/main.rs`
   - **Implementation:**
     ```rust
     pub fn run() -> Result<(), Box<dyn std::error::Error>> {
         let cli = Cli::parse();

         // Set NO_COLOR env var if flag is passed
         if cli.global.no_color {
             std::env::set_var("NO_COLOR", "1");
         }

         // ... rest of function
     }
     ```

5. **Add progress indicators for long operations** (Issue 5)
   - Emit periodic progress to stderr for screen readers
   - Detect TTY and use appropriate format (progress bar vs. line updates)
   - **Effort:** Medium (4-6 hours)
   - **Files:** Streaming operations in `/home/kautau/work/personal/code/base-d/src/encoders/streaming/`

6. **Add JSON output to detect command** (Issue 7)
   - Consistent with config command pattern
   - **Effort:** Low (2 hours)
   - **File:** `/home/kautau/work/personal/code/base-d/src/cli/handlers/detect.rs`

### Enhancement (Level AAA / Nice-to-have)

7. **Add audio description mode for Matrix mode** (Issue 6)
   - `--describe` flag that narrates visual changes
   - "Now showing base64... switching to hieroglyphics..."
   - **Effort:** Medium (4-6 hours)
   - **Priority:** Low - Matrix mode is decorative

8. **Add command aliases for motor accessibility** (Issue 8)
   - Short aliases for all commands
   - **Effort:** Low (1 hour)
   - **File:** `/home/kautau/work/personal/code/base-d/src/cli/mod.rs`

9. **Improve error message clarity** (Issue 9)
   - Review all error messages for plain language
   - **Effort:** Medium (3-4 hours across codebase)
   - **Priority:** Low - current errors are reasonable

10. **Terminal color contrast documentation** (Color Contrast Audit)
    - Document that users should use high-contrast terminal themes
    - Link to terminal accessibility guides
    - **Effort:** Low (30 minutes)

## What's Good

base-d demonstrates several accessibility best practices worth maintaining:

1. **NO_COLOR compliance** - Properly implements NO_COLOR environment variable with terminal detection
2. **Dual output paths** - All colored output has plain-text equivalent
3. **Keyboard-only operation** - No mouse required anywhere in the tool
4. **Machine-readable output** - JSON output options for automated processing
5. **Clear error messages** - Dictionary suggestions via fuzzy matching
6. **Graceful degradation** - Features work without color, visual effects are optional
7. **Standard streams** - Proper use of stdout/stderr separation
8. **Terminal capability detection** - Uses `IsTerminal` to avoid ANSI codes in pipes
9. **Streaming architecture** - Memory-efficient for large files
10. **Comprehensive keyboard controls** - Matrix mode has thoughtful keyboard navigation
11. **No time limits** - No operations have time constraints (good for users who need more time)
12. **Clear command structure** - Verb-noun pattern is intuitive

**Architectural strength:** The separation between library code and CLI code, combined with proper stderr/stdout handling and terminal detection, creates a solid foundation for accessibility. The issues identified are mostly about refinement and documentation rather than fundamental problems.

## Testing Recommendations

To verify accessibility improvements:

1. **Screen reader testing:**
   - Install NVDA (Windows, free) or use VoiceOver (macOS, built-in)
   - Test basic commands: `echo "test" | base-d encode base64 | base-d decode base64`
   - Verify error messages are announced
   - Test Matrix mode exit (should announce "exiting" or similar)

2. **NO_COLOR testing:**
   ```bash
   NO_COLOR=1 base-d encode base64 < /dev/urandom | head -c 1000 | base-d detect
   base-d --no-color encode base64 < /dev/urandom | head -c 1000 | base-d detect
   ```

3. **Keyboard-only testing:**
   - Use terminal without mouse
   - Test all Matrix mode controls
   - Verify tab completion works (if implemented)

4. **Machine parsing testing:**
   ```bash
   base-d config list --json | jq '.dictionaries | length'
   ```

5. **Progress indicator testing** (after implementation):
   - Generate large file: `dd if=/dev/urandom of=large.bin bs=1M count=100`
   - Encode with progress: `base-d encode base64 -f large.bin` (should show progress)

**Knock knock, Neo.**
