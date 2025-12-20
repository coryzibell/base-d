# Custom Dictionaries

base-d supports loading custom dictionaries from configuration files, allowing you to define your own encoding schemes without modifying the source code.

## Configuration File Locations

base-d loads dictionaries from multiple locations in this order:

1. **Built-in dictionaries** - 55 pre-configured dictionaries
2. **User config** - `~/.config/base-d/dictionaries.toml` (Linux/macOS) or `%APPDATA%\base-d\dictionaries.toml` (Windows)
3. **Local config** - `./dictionaries.toml` in the current directory

Later configurations override earlier ones, so you can:
- Add new dictionaries
- Replace built-in dictionaries with your own versions

## Configuration Format

base-d supports two types of dictionaries:

1. **Character-based** (default) - Each symbol is a single character
2. **Word-based** - Each symbol is a whole word (like BIP-39 mnemonics)

### Character-based Dictionaries

```toml
[dictionaries.my_dictionary]
chars = "0123456789ABCDEF"
mode = "base_conversion"  # or "chunked" or "byte_range"
padding = "="  # optional, only for chunked mode

[dictionaries.my_range]
mode = "byte_range"
start_codepoint = 128000  # Unicode codepoint for first character
```

### Word-based Dictionaries

```toml
[dictionaries.my_words]
type = "word"
words = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"]
delimiter = " "          # separator between words (default: space)
case_sensitive = false   # match words case-insensitively (default: false)

[dictionaries.from_file]
type = "word"
words_file = "/path/to/wordlist.txt"  # one word per line
delimiter = "-"
```

Word dictionaries use radix (base) conversion, where each "digit" is a word from your list.

## Encoding Modes

### Mathematical Base Conversion (`base_conversion`)

Default mode that treats data as a large number and converts to the target base.

```toml
[dictionaries.custom_hex]
chars = "0123456789abcdef"
mode = "base_conversion"
```

Works with any dictionary size. No padding required.

### RFC 4648 Chunked Mode (`chunked`)

Processes data in fixed-size bit groups, compatible with standard encodings.

```toml
[dictionaries.my_base32]
chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
mode = "chunked"
padding = "="
```

Requirements:
- Dictionary size must be a power of 2 (2, 4, 8, 16, 32, 64, etc.)
- Optional padding character

### Byte Range Mode (`byte_range`)

Direct 1:1 byte-to-character mapping using a Unicode range.

```toml
[dictionaries.my_emoji]
mode = "byte_range"
start_codepoint = 127991  # Each byte maps to start + byte_value
```

Perfect for emoji or other continuous Unicode ranges. Zero encoding overhead.

## Examples

### Example 1: Custom Emoji Dictionary

Create `~/.config/base-d/dictionaries.toml`:

```toml
[dictionaries.happy_emoji]
chars = "😀😁😂🤣😃😄😅😆😉😊😋😎😍😘🥰😗"
mode = "base_conversion"
```

Usage:
```bash
$ echo "Hi" | base-d -e happy_emoji
😍😁
```

### Example 2: Override Built-in Dictionary

Replace the default `binary` dictionary with your own:

```toml
[dictionaries.binary]
chars = "🔵🔴"  # Use colored circles instead of 0 and 1
mode = "base_conversion"
```

### Example 3: Project-Specific Dictionary

Create `./dictionaries.toml` in your project directory:

```toml
[dictionaries.project]
chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
mode = "base_conversion"
```

This dictionary will only be available when running base-d from this directory.

### Example 4: Custom Base100 Range

```toml
[dictionaries.symbols]
mode = "byte_range"
start_codepoint = 9728  # Weather symbols ☀☁☂...
```

### Example 5: Custom Word Dictionary

Create a NATO phonetic alphabet dictionary:

```toml
[dictionaries.nato]
type = "word"
words = [
    "alfa", "bravo", "charlie", "delta", "echo", "foxtrot",
    "golf", "hotel", "india", "juliet", "kilo", "lima",
    "mike", "november", "oscar", "papa", "quebec", "romeo",
    "sierra", "tango", "uniform", "victor", "whiskey", "xray",
    "yankee", "zulu"
]
delimiter = "-"
case_sensitive = false
```

Usage:
```bash
$ echo "Hi" | base-d encode nato
bravo-bravo-lima-golf
$ echo "bravo-bravo-lima-golf" | base-d decode nato
Hi
```

### Example 6: Word Dictionary from File

Use an external word list:

```toml
[dictionaries.diceware]
type = "word"
words_file = "~/.config/base-d/diceware.txt"
delimiter = " "
```

The file should contain one word per line.

## Validation

base-d automatically validates custom dictionaries:

**Character-based:**
- **Duplicate characters**: Each character must appear only once
- **Empty dictionaries**: At least one character required (except byte_range mode)
- **Chunked mode**: Dictionary size must be power of 2
- **Byte range**: Start codepoint must allow 256 valid Unicode characters

**Word-based:**
- **Duplicate words**: Each word must be unique (respecting case_sensitive setting)
- **Empty word lists**: At least one word required
- **No empty words**: Each word must contain at least one character

Errors are reported with helpful messages:

```bash
$ echo "test" | base-d -e invalid
Error: Invalid dictionary: Duplicate character in dictionary: A
```

## Tips

1. **Test your dictionaries**: Always test encode/decode round-trips
   ```bash
   echo "test" | base-d -e my_dictionary | base-d -d my_dictionary
   ```

2. **Avoid ambiguous characters**: Don't use characters that look similar (0/O, 1/l/I)

3. **Consider your use case**:
   - Mathematical mode: Maximum flexibility, any dictionary size
   - Chunked mode: RFC compliance, standard tooling compatibility
   - Byte range: Maximum efficiency, continuous Unicode blocks

4. **Version control**: Check in project-specific `dictionaries.toml` files

5. **Word dictionaries for humans**: Use word-based dictionaries when output needs to be read aloud, written down, or memorized (like seed phrases)

## Troubleshooting

### Config not loading

Check file permissions and path:
```bash
# Linux/macOS
ls -l ~/.config/base-d/dictionaries.toml

# Windows
dir %APPDATA%\base-d\dictionaries.toml
```

### Dictionary not found

List available dictionaries to verify it loaded:
```bash
base-d --list | grep my_dictionary
```

### Invalid TOML syntax

base-d will show TOML parsing errors. Common issues:
- Missing quotes around strings
- Incorrect indentation
- Unclosed brackets

Use a TOML validator or editor with TOML support.

## See Also

- [DICTIONARIES.md](DICTIONARIES.md) - Reference for all built-in dictionaries
- [ENCODING_MODES.md](ENCODING_MODES.md) - Detailed explanation of encoding modes
- `dictionaries.toml` - Source configuration for built-in dictionaries
