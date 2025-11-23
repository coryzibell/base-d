# Custom Alphabets

base-d supports loading custom alphabets from configuration files, allowing you to define your own encoding schemes without modifying the source code.

## Configuration File Locations

base-d loads alphabets from multiple locations in this order:

1. **Built-in alphabets** - 33 pre-configured alphabets
2. **User config** - `~/.config/base-d/alphabets.toml` (Linux/macOS) or `%APPDATA%\base-d\alphabets.toml` (Windows)
3. **Local config** - `./alphabets.toml` in the current directory

Later configurations override earlier ones, so you can:
- Add new alphabets
- Replace built-in alphabets with your own versions

## Configuration Format

Custom alphabets use the same TOML format as the built-in configuration:

```toml
[alphabets.my_alphabet]
chars = "0123456789ABCDEF"
mode = "base_conversion"  # or "chunked" or "byte_range"
padding = "="  # optional, only for chunked mode

[alphabets.my_range]
mode = "byte_range"
start_codepoint = 128000  # Unicode codepoint for first character
```

## Encoding Modes

### Mathematical Base Conversion (`base_conversion`)

Default mode that treats data as a large number and converts to the target base.

```toml
[alphabets.custom_hex]
chars = "0123456789abcdef"
mode = "base_conversion"
```

Works with any alphabet size. No padding required.

### RFC 4648 Chunked Mode (`chunked`)

Processes data in fixed-size bit groups, compatible with standard encodings.

```toml
[alphabets.my_base32]
chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
mode = "chunked"
padding = "="
```

Requirements:
- Alphabet size must be a power of 2 (2, 4, 8, 16, 32, 64, etc.)
- Optional padding character

### Byte Range Mode (`byte_range`)

Direct 1:1 byte-to-character mapping using a Unicode range.

```toml
[alphabets.my_emoji]
mode = "byte_range"
start_codepoint = 127991  # Each byte maps to start + byte_value
```

Perfect for emoji or other continuous Unicode ranges. Zero encoding overhead.

## Examples

### Example 1: Custom Emoji Alphabet

Create `~/.config/base-d/alphabets.toml`:

```toml
[alphabets.happy_emoji]
chars = "😀😁😂🤣😃😄😅😆😉😊😋😎😍😘🥰😗"
mode = "base_conversion"
```

Usage:
```bash
$ echo "Hi" | base-d -a happy_emoji
😍😁
```

### Example 2: Override Built-in Alphabet

Replace the default `binary` alphabet with your own:

```toml
[alphabets.binary]
chars = "🔵🔴"  # Use colored circles instead of 0 and 1
mode = "base_conversion"
```

### Example 3: Project-Specific Alphabet

Create `./alphabets.toml` in your project directory:

```toml
[alphabets.project]
chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
mode = "base_conversion"
```

This alphabet will only be available when running base-d from this directory.

### Example 4: Custom Base100 Range

```toml
[alphabets.symbols]
mode = "byte_range"
start_codepoint = 9728  # Weather symbols ☀☁☂...
```

## Validation

base-d automatically validates custom alphabets:

- **Duplicate characters**: Each character must appear only once
- **Empty alphabets**: At least one character required (except byte_range mode)
- **Chunked mode**: Alphabet size must be power of 2
- **Byte range**: Start codepoint must allow 256 valid Unicode characters

Errors are reported with helpful messages:

```bash
$ echo "test" | base-d -a invalid
Error: Invalid alphabet: Duplicate character in alphabet: A
```

## Tips

1. **Test your alphabets**: Always test encode/decode round-trips
   ```bash
   echo "test" | base-d -a my_alphabet | base-d -a my_alphabet -d
   ```

2. **Avoid ambiguous characters**: Don't use characters that look similar (0/O, 1/l/I)

3. **Consider your use case**:
   - Mathematical mode: Maximum flexibility, any alphabet size
   - Chunked mode: RFC compliance, standard tooling compatibility
   - Byte range: Maximum efficiency, continuous Unicode blocks

4. **Version control**: Check in project-specific `alphabets.toml` files

## Troubleshooting

### Config not loading

Check file permissions and path:
```bash
# Linux/macOS
ls -l ~/.config/base-d/alphabets.toml

# Windows
dir %APPDATA%\base-d\alphabets.toml
```

### Alphabet not found

List available alphabets to verify it loaded:
```bash
base-d --list | grep my_alphabet
```

### Invalid TOML syntax

base-d will show TOML parsing errors. Common issues:
- Missing quotes around strings
- Incorrect indentation
- Unclosed brackets

Use a TOML validator or editor with TOML support.

## See Also

- [ALPHABETS.md](ALPHABETS.md) - Reference for all built-in alphabets
- [ENCODING_MODES.md](ENCODING_MODES.md) - Detailed explanation of encoding modes
- `alphabets.toml` - Source configuration for built-in alphabets
