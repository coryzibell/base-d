//! # base-d
//!
//! A universal, multi-dictionary encoding library for Rust.
//!
//! Encode binary data using numerous dictionaries including RFC standards, ancient scripts,
//! emoji, playing cards, and more. Supports three encoding modes: radix (true base
//! conversion), RFC 4648 chunked encoding, and direct byte-range mapping.
//!
//! ## Quick Start
//!
//! ```
//! use base_d::{DictionaryRegistry, Dictionary, encode, decode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load built-in dictionaries
//! let config = DictionaryRegistry::load_default()?;
//! let base64_config = config.get_dictionary("base64").unwrap();
//!
//! // Create dictionary
//! let chars: Vec<char> = base64_config.chars.chars().collect();
//! let padding = base64_config.padding.as_ref().and_then(|s| s.chars().next());
//! let mut builder = Dictionary::builder()
//!     .chars(chars)
//!     .mode(base64_config.effective_mode());
//! if let Some(p) = padding {
//!     builder = builder.padding(p);
//! }
//! let dictionary = builder.build()?;
//!
//! // Encode and decode
//! let data = b"Hello, World!";
//! let encoded = encode(data, &dictionary);
//! let decoded = decode(&encoded, &dictionary)?;
//! assert_eq!(data, &decoded[..]);
//! # Ok(())
//! # }
//! ```
//!
//! ## Features
//!
//! - **33 Built-in Dictionaries**: RFC standards, emoji, ancient scripts, and more
//! - **3 Encoding Modes**: Radix, chunked (RFC-compliant), byte-range
//! - **Streaming Support**: Memory-efficient processing for large files
//! - **Custom Dictionaries**: Define your own via TOML configuration
//! - **User Configuration**: Load dictionaries from `~/.config/base-d/dictionaries.toml`
//! - **SIMD Acceleration**: AVX2/SSSE3 on x86_64, NEON on aarch64 (enabled by default)
//!
//! ## Cargo Features
//!
//! - `simd` (default): Enable SIMD acceleration for encoding/decoding.
//!   Disable with `--no-default-features` for scalar-only builds.
//!
//! ## Encoding Modes
//!
//! ### Radix Base Conversion
//!
//! True base conversion treating data as a large number. Works with any dictionary size.
//!
//! ```
//! use base_d::{Dictionary, EncodingMode, encode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let chars: Vec<char> = "😀😁😂🤣😃😄😅😆".chars().collect();
//! let dictionary = Dictionary::builder()
//!     .chars(chars)
//!     .mode(EncodingMode::Radix)
//!     .build()?;
//!
//! let encoded = encode(b"Hi", &dictionary);
//! # Ok(())
//! # }
//! ```
//!
//! ### Chunked Mode (RFC 4648)
//!
//! Fixed-size bit groups, compatible with standard base64/base32.
//!
//! ```
//! use base_d::{Dictionary, EncodingMode, encode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
//!     .chars().collect();
//! let dictionary = Dictionary::builder()
//!     .chars(chars)
//!     .mode(EncodingMode::Chunked)
//!     .padding('=')
//!     .build()?;
//!
//! let encoded = encode(b"Hello", &dictionary);
//! assert_eq!(encoded, "SGVsbG8=");
//! # Ok(())
//! # }
//! ```
//!
//! ### Byte Range Mode
//!
//! Direct 1:1 byte-to-emoji mapping. Zero encoding overhead.
//!
//! ```
//! use base_d::{Dictionary, EncodingMode, encode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let dictionary = Dictionary::builder()
//!     .mode(EncodingMode::ByteRange)
//!     .start_codepoint(127991)  // U+1F3F7
//!     .build()?;
//!
//! let data = b"Hi";
//! let encoded = encode(data, &dictionary);
//! assert_eq!(encoded.chars().count(), 2);  // 1:1 mapping
//! # Ok(())
//! # }
//! ```
//!
//! ## Streaming
//!
//! For large files, use streaming to avoid loading entire file into memory:
//!
//! ```no_run
//! use base_d::{DictionaryRegistry, StreamingEncoder};
//! use std::fs::File;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = DictionaryRegistry::load_default()?;
//! let dictionary_config = config.get_dictionary("base64").unwrap();
//!
//! // ... create dictionary from config
//! # let chars: Vec<char> = dictionary_config.chars.chars().collect();
//! # let padding = dictionary_config.padding.as_ref().and_then(|s| s.chars().next());
//! # let mut builder = base_d::Dictionary::builder().chars(chars).mode(dictionary_config.effective_mode());
//! # if let Some(p) = padding { builder = builder.padding(p); }
//! # let dictionary = builder.build()?;
//!
//! let mut input = File::open("large_file.bin")?;
//! let output = File::create("encoded.txt")?;
//!
//! let mut encoder = StreamingEncoder::new(&dictionary, output);
//! encoder.encode(&mut input)?;
//! # Ok(())
//! # }
//! ```

mod core;
mod encoders;
mod features;

#[cfg(feature = "simd")]
mod simd;

pub mod bench;
pub mod convenience;
pub mod prelude;
pub mod wordlists;

pub use convenience::{
    CompressEncodeResult, HashEncodeResult, compress_encode, compress_encode_with, hash_encode,
    hash_encode_with,
};
pub use core::config::{
    CompressionConfig, DictionaryConfig, DictionaryRegistry, DictionaryType, EncodingMode, Settings,
};
pub use core::alternating_dictionary::AlternatingWordDictionary;
pub use core::dictionary::{Dictionary, DictionaryBuilder};
pub use core::word_dictionary::{WordDictionary, WordDictionaryBuilder};
pub use encoders::algorithms::{DecodeError, DictionaryNotFoundError, find_closest_dictionary};

/// Word-based encoding using radix conversion.
///
/// Same mathematical approach as character-based radix encoding,
/// but outputs words joined by a delimiter instead of concatenated characters.
pub mod word {
    pub use crate::encoders::algorithms::word::{decode, encode};
}

/// Alternating word-based encoding for PGP-style biometric word lists.
///
/// Provides direct 1:1 byte-to-word mapping where the dictionary selection
/// alternates based on byte position (e.g., even/odd bytes use different dictionaries).
pub mod word_alternating {
    pub use crate::encoders::algorithms::word_alternating::{decode, encode};
}
pub use encoders::streaming::{StreamingDecoder, StreamingEncoder};

// Expose schema encoding functions for CLI
pub use encoders::algorithms::schema::{
    SchemaCompressionAlgo, decode_fiche, decode_fiche_path, decode_schema, encode_fiche,
    encode_fiche_ascii, encode_fiche_light, encode_fiche_minified, encode_fiche_path,
    encode_fiche_readable, encode_markdown_fiche, encode_markdown_fiche_ascii,
    encode_markdown_fiche_light, encode_markdown_fiche_markdown, encode_markdown_fiche_readable,
    encode_schema,
};

// Expose fiche auto-detection
pub use encoders::algorithms::schema::fiche_analyzer::{DetectedMode, detect_fiche_mode};

/// Schema encoding types and traits for building custom frontends
///
/// This module provides the intermediate representation (IR) layer for schema encoding,
/// allowing library users to implement custom parsers (YAML, CSV, TOML, etc.) and
/// serializers that leverage the binary encoding backend.
///
/// # Architecture
///
/// The schema encoding pipeline has three layers:
///
/// 1. **Input layer**: Parse custom formats into IR
///    - Implement `InputParser` trait
///    - Reference: `JsonParser`
///
/// 2. **Binary layer**: Pack/unpack IR to/from binary
///    - `pack()` - IR to binary bytes
///    - `unpack()` - Binary bytes to IR
///    - `encode_framed()` - Binary to display96 with delimiters
///    - `decode_framed()` - Display96 to binary
///
/// 3. **Output layer**: Serialize IR to custom formats
///    - Implement `OutputSerializer` trait
///    - Reference: `JsonSerializer`
///
/// # Example: Custom CSV Parser
///
/// ```ignore
/// use base_d::schema::{
///     InputParser, IntermediateRepresentation, SchemaHeader, FieldDef,
///     FieldType, SchemaValue, SchemaError, pack, encode_framed,
/// };
///
/// struct CsvParser;
///
/// impl InputParser for CsvParser {
///     type Error = SchemaError;
///
///     fn parse(input: &str) -> Result<IntermediateRepresentation, Self::Error> {
///         // Parse CSV headers
///         let lines: Vec<&str> = input.lines().collect();
///         let headers: Vec<&str> = lines[0].split(',').collect();
///
///         // Infer types and build fields
///         let fields: Vec<FieldDef> = headers.iter()
///             .map(|h| FieldDef::new(h.to_string(), FieldType::String))
///             .collect();
///
///         // Parse rows
///         let row_count = lines.len() - 1;
///         let mut values = Vec::new();
///         for line in &lines[1..] {
///             for cell in line.split(',') {
///                 values.push(SchemaValue::String(cell.to_string()));
///             }
///         }
///
///         let header = SchemaHeader::new(row_count, fields);
///         IntermediateRepresentation::new(header, values)
///     }
/// }
///
/// // Encode CSV to schema format
/// let csv = "name,age\nalice,30\nbob,25";
/// let ir = CsvParser::parse(csv)?;
/// let binary = pack(&ir);
/// let encoded = encode_framed(&binary);
/// ```
///
/// # IR Structure
///
/// The `IntermediateRepresentation` consists of:
///
/// * **Header**: Schema metadata
///   - Field definitions (name + type)
///   - Row count
///   - Optional root key
///   - Optional null bitmap
///
/// * **Values**: Flat array in row-major order
///   - `[row0_field0, row0_field1, row1_field0, row1_field1, ...]`
///
/// # Type System
///
/// Supported field types:
///
/// * `U64` - Unsigned 64-bit integer
/// * `I64` - Signed 64-bit integer
/// * `F64` - 64-bit floating point
/// * `String` - UTF-8 string
/// * `Bool` - Boolean
/// * `Null` - Null value
/// * `Array(T)` - Homogeneous array of type T
/// * `Any` - Mixed-type values
///
/// # Compression
///
/// Optional compression algorithms:
///
/// * `SchemaCompressionAlgo::Brotli` - Best ratio
/// * `SchemaCompressionAlgo::Lz4` - Fastest
/// * `SchemaCompressionAlgo::Zstd` - Balanced
///
/// # See Also
///
/// * [SCHEMA.md](../SCHEMA.md) - Full format specification
/// * `encode_schema()` / `decode_schema()` - High-level JSON functions
pub mod schema {
    pub use crate::encoders::algorithms::schema::{
        // IR types
        FieldDef,
        FieldType,
        // Traits
        InputParser,
        IntermediateRepresentation,
        // Reference implementations
        JsonParser,
        JsonSerializer,
        OutputSerializer,
        // Compression
        SchemaCompressionAlgo,
        // Errors
        SchemaError,
        SchemaHeader,
        SchemaValue,
        // Binary layer
        decode_framed,
        // High-level API
        decode_schema,
        encode_framed,
        encode_schema,
        pack,
        unpack,
    };
}
pub use features::{
    CompressionAlgorithm, DictionaryDetector, DictionaryMatch, HashAlgorithm, XxHashConfig,
    compress, decompress, detect_dictionary, hash, hash_with_config,
};

/// Encodes binary data using the specified dictionary.
///
/// Automatically selects the appropriate encoding strategy based on the
/// dictionary's mode (Radix, Chunked, or ByteRange).
///
/// # Arguments
///
/// * `data` - The binary data to encode
/// * `dictionary` - The dictionary to use for encoding
///
/// # Returns
///
/// A string containing the encoded data
///
/// # Examples
///
/// ```
/// use base_d::{Dictionary, EncodingMode};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let chars: Vec<char> = "01".chars().collect();
/// let dictionary = Dictionary::builder()
///     .chars(chars)
///     .mode(EncodingMode::Radix)
///     .build()?;
/// let encoded = base_d::encode(b"Hi", &dictionary);
/// # Ok(())
/// # }
/// ```
pub fn encode(data: &[u8], dictionary: &Dictionary) -> String {
    match dictionary.mode() {
        EncodingMode::Radix => encoders::algorithms::radix::encode(data, dictionary),
        EncodingMode::Chunked => encoders::algorithms::chunked::encode_chunked(data, dictionary),
        EncodingMode::ByteRange => {
            encoders::algorithms::byte_range::encode_byte_range(data, dictionary)
        }
    }
}

/// Decodes a string back to binary data using the specified dictionary.
///
/// Automatically selects the appropriate decoding strategy based on the
/// dictionary's mode (Radix, Chunked, or ByteRange).
///
/// # Arguments
///
/// * `encoded` - The encoded string to decode
/// * `dictionary` - The dictionary used for encoding
///
/// # Returns
///
/// A `Result` containing the decoded binary data, or a `DecodeError` if
/// the input is invalid
///
/// # Errors
///
/// Returns `DecodeError` if:
/// - The input contains invalid characters
/// - The input is empty
/// - The padding is invalid (for chunked mode)
///
/// # Examples
///
/// ```
/// use base_d::{Dictionary, EncodingMode, encode, decode};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let chars: Vec<char> = "01".chars().collect();
/// let dictionary = Dictionary::builder()
///     .chars(chars)
///     .mode(EncodingMode::Radix)
///     .build()?;
/// let data = b"Hi";
/// let encoded = encode(data, &dictionary);
/// let decoded = decode(&encoded, &dictionary)?;
/// assert_eq!(data, &decoded[..]);
/// # Ok(())
/// # }
/// ```
pub fn decode(encoded: &str, dictionary: &Dictionary) -> Result<Vec<u8>, DecodeError> {
    match dictionary.mode() {
        EncodingMode::Radix => encoders::algorithms::radix::decode(encoded, dictionary),
        EncodingMode::Chunked => encoders::algorithms::chunked::decode_chunked(encoded, dictionary),
        EncodingMode::ByteRange => {
            encoders::algorithms::byte_range::decode_byte_range(encoded, dictionary)
        }
    }
}

#[cfg(test)]
mod tests;
