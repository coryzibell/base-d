# Library API Reference

Complete API reference for using base-d as a Rust library.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
base-d = "0.1"
```

## Core Types

### `Dictionary`

Represents an encoding dictionary with associated encoding mode.

```rust
use base_d::{Dictionary, EncodingMode};

// Create from character vector
let chars: Vec<char> = "0123456789ABCDEF".chars().collect();
let dictionary = Dictionary::new(chars)?;

// Create with specific mode
let dictionary = Dictionary::new_with_mode(
    chars,
    EncodingMode::Chunked,
    Some('=')  // padding character
)?;

// Create byte-range dictionary
let dictionary = Dictionary::new_with_mode_and_range(
    Vec::new(),
    EncodingMode::ByteRange,
    None,
    Some(127991)  // start codepoint
)?;
```

**Methods:**
- `base(&self) -> usize` - Get dictionary size
- `mode(&self) -> &EncodingMode` - Get encoding mode
- `encode_digit(&self, digit: usize) -> Option<char>` - Encode single digit
- `decode_char(&self, c: char) -> Option<usize>` - Decode single character

### `EncodingMode`

Encoding algorithm to use.

```rust
pub enum EncodingMode {
    BaseConversion,  // Mathematical (any dictionary size)
    Chunked,         // RFC 4648 (power-of-2 sizes)
    ByteRange,       // Direct byte-to-char (256 chars)
}
```

### `DictionariesConfig`

Configuration containing multiple dictionary definitions.

```rust
use base_d::DictionariesConfig;

// Load built-in dictionaries
let config = DictionariesConfig::load_default()?;

// Load with user overrides
let config = DictionariesConfig::load_with_overrides()?;

// Load from specific file
let config = DictionariesConfig::load_from_file("config.toml".as_ref())?;

// Get specific dictionary
let base64_config = config.get_dictionary("base64").unwrap();

// Merge configurations
let mut config1 = DictionariesConfig::load_default()?;
let config2 = DictionariesConfig::load_from_file("custom.toml".as_ref())?;
config1.merge(config2);
```

### `DictionaryConfig`

Configuration for a single dictionary.

```rust
pub struct DictionaryConfig {
    pub chars: String,                    // Dictionary characters
    pub mode: EncodingMode,               // Encoding mode
    pub padding: Option<String>,          // Padding character
    pub start_codepoint: Option<u32>,     // For ByteRange mode
}
```

### `DecodeError`

Error type for decoding operations.

```rust
pub enum DecodeError {
    InvalidCharacter(char),  // Character not in dictionary
    EmptyInput,              // Empty string provided
    InvalidPadding,          // Malformed padding
}
```

Implements `std::error::Error` and `Display`.

## Encoding Functions

### `encode`

Encode bytes to string using specified dictionary.

```rust
use base_d::{encode, Dictionary};

let dictionary = /* create dictionary */;
let data = b"Hello, World!";
let encoded: String = encode(data, &dictionary);
```

### `decode`

Decode string to bytes using specified dictionary.

```rust
use base_d::{decode, Dictionary, DecodeError};

let dictionary = /* create dictionary */;
let encoded = "SGVsbG8sIFdvcmxkIQ==";
let decoded: Result<Vec<u8>, DecodeError> = decode(encoded, &dictionary);
```

## Streaming API

### `StreamingEncoder`

Encode data in chunks without loading entire input.

```rust
use base_d::{StreamingEncoder, Dictionary};
use std::fs::File;

let dictionary = /* create dictionary */;
let mut input = File::open("input.bin")?;
let mut output = File::create("output.txt")?;

let mut encoder = StreamingEncoder::new(&dictionary, output);
encoder.encode(&mut input)?;
```

**Methods:**
- `new(dictionary: &Dictionary, writer: W) -> Self` - Create encoder
- `encode<R: Read>(&mut self, reader: &mut R) -> std::io::Result<()>` - Encode stream

### `StreamingDecoder`

Decode data in chunks without loading entire input.

```rust
use base_d::{StreamingDecoder, Dictionary, DecodeError};
use std::fs::File;

let dictionary = /* create dictionary */;
let mut input = File::open("input.txt")?;
let mut output = File::create("output.bin")?;

let mut decoder = StreamingDecoder::new(&dictionary, output);
decoder.decode(&mut input)?;
```

**Methods:**
- `new(dictionary: &Dictionary, writer: W) -> Self` - Create decoder
- `decode<R: Read>(&mut self, reader: &mut R) -> Result<(), DecodeError>` - Decode stream

## Complete Examples

### Example 1: Simple Encoding

```rust
use base_d::{DictionariesConfig, Dictionary, encode, decode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DictionariesConfig::load_default()?;
    let base64_config = config.get_dictionary("base64").unwrap();
    
    let chars: Vec<char> = base64_config.chars.chars().collect();
    let padding = base64_config.padding.as_ref().and_then(|s| s.chars().next());
    let dictionary = Dictionary::new_with_mode(
        chars,
        base64_config.mode.clone(),
        padding
    )?;
    
    let data = b"Hello, World!";
    let encoded = encode(data, &dictionary);
    println!("Encoded: {}", encoded);
    
    let decoded = decode(&encoded, &dictionary)?;
    assert_eq!(data, &decoded[..]);
    
    Ok(())
}
```

### Example 2: Custom Dictionary

```rust
use base_d::{Dictionary, EncodingMode, encode, decode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create custom emoji dictionary
    let chars: Vec<char> = "😀😁😂🤣😃😄😅😆".chars().collect();
    let dictionary = Dictionary::new_with_mode(
        chars,
        EncodingMode::BaseConversion,
        None
    )?;
    
    let data = b"Hi";
    let encoded = encode(data, &dictionary);
    println!("Encoded: {}", encoded);
    
    let decoded = decode(&encoded, &dictionary)?;
    assert_eq!(data, &decoded[..]);
    
    Ok(())
}
```

### Example 3: ByteRange (base100)

```rust
use base_d::{Dictionary, EncodingMode, encode, decode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create base100 dictionary (256 emoji)
    let dictionary = Dictionary::new_with_mode_and_range(
        Vec::new(),
        EncodingMode::ByteRange,
        None,
        Some(127991)  // U+1F3F7
    )?;
    
    let data = b"Test";
    let encoded = encode(data, &dictionary);
    println!("Encoded: {}", encoded);  // 4 emoji, 1:1 mapping
    
    let decoded = decode(&encoded, &dictionary)?;
    assert_eq!(data, &decoded[..]);
    
    Ok(())
}
```

### Example 4: Streaming Large Files

```rust
use base_d::{DictionariesConfig, Dictionary, StreamingEncoder, StreamingDecoder};
use std::fs::File;

fn stream_encode_file(
    input_path: &str,
    output_path: &str,
    alphabet_name: &str
) -> Result<(), Box<dyn std::error::Error>> {
    let config = DictionariesConfig::load_default()?;
    let alphabet_config = config.get_dictionary(alphabet_name)
        .ok_or("Dictionary not found")?;
    
    let dictionary = create_alphabet_from_config(alphabet_config)?;
    
    let mut input = File::open(input_path)?;
    let output = File::create(output_path)?;
    
    let mut encoder = StreamingEncoder::new(&dictionary, output);
    encoder.encode(&mut input)?;
    
    Ok(())
}

fn create_alphabet_from_config(
    config: &base_d::DictionaryConfig
) -> Result<Dictionary, Box<dyn std::error::Error>> {
    use base_d::EncodingMode;
    
    match config.mode {
        EncodingMode::ByteRange => {
            let start = config.start_codepoint
                .ok_or("ByteRange requires start_codepoint")?;
            Ok(Dictionary::new_with_mode_and_range(
                Vec::new(),
                config.mode.clone(),
                None,
                Some(start)
            )?)
        }
        _ => {
            let chars: Vec<char> = config.chars.chars().collect();
            let padding = config.padding.as_ref()
                .and_then(|s| s.chars().next());
            Ok(Dictionary::new_with_mode(
                chars,
                config.mode.clone(),
                padding
            )?)
        }
    }
}
```

### Example 5: Error Handling

```rust
use base_d::{decode, Dictionary, DecodeError};

fn safe_decode(
    encoded: &str,
    dictionary: &Dictionary
) -> Result<Vec<u8>, String> {
    decode(encoded, dictionary).map_err(|e| match e {
        DecodeError::InvalidCharacter(c) => {
            format!("Invalid character '{}' in encoded data", c)
        }
        DecodeError::EmptyInput => {
            "Cannot decode empty string".to_string()
        }
        DecodeError::InvalidPadding => {
            "Malformed padding in encoded data".to_string()
        }
    })
}
```

### Example 6: Working with User Config

```rust
use base_d::DictionariesConfig;

fn list_available_alphabets() -> Result<(), Box<dyn std::error::Error>> {
    // Load with user overrides
    let config = DictionariesConfig::load_with_overrides()?;
    
    println!("Available dictionaries:");
    for (name, alphabet_config) in config.dictionaries.iter() {
        let mode = match alphabet_config.mode {
            base_d::EncodingMode::BaseConversion => "math",
            base_d::EncodingMode::Chunked => "chunked",
            base_d::EncodingMode::ByteRange => "range",
        };
        
        let size = match alphabet_config.mode {
            base_d::EncodingMode::ByteRange => 256,
            _ => alphabet_config.chars.chars().count(),
        };
        
        println!("  {} (base-{}, {})", name, size, mode);
    }
    
    Ok(())
}
```

## Thread Safety

All types are `Send` and `Sync` where appropriate:
- `Dictionary` is `Send + Sync`
- `DictionariesConfig` is `Send + Sync`
- `StreamingEncoder` and `StreamingDecoder` are `Send` (not `Sync` due to `Write` requirement)

## Performance Tips

1. **Reuse Dictionaries**: Create dictionary once and reuse for multiple operations
2. **Use Streaming**: For files > 10MB, use `StreamingEncoder`/`StreamingDecoder`
3. **Choose Right Mode**: 
   - Chunked: Best for RFC compliance and streaming
   - ByteRange: Best for emoji/1:1 mapping
   - BaseConversion: Most flexible but slowest
4. **Avoid String Allocations**: `encode` returns `String`, consider using streaming for large data

## See Also

- [DICTIONARIES.md](DICTIONARIES.md) - All built-in dictionaries
- [ENCODING_MODES.md](ENCODING_MODES.md) - Encoding algorithm details
- [STREAMING.md](STREAMING.md) - Streaming guide
- [CUSTOM_DICTIONARIES.md](CUSTOM_DICTIONARIES.md) - Creating custom dictionaries
