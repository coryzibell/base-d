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
//!     .mode(base64_config.mode.clone());
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
//! # let mut builder = base_d::Dictionary::builder().chars(chars).mode(dictionary_config.mode.clone());
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

mod simd;

pub use core::config::{
    CompressionConfig, DictionaryConfig, DictionaryRegistry, EncodingMode, Settings,
};
pub use core::dictionary::{Dictionary, DictionaryBuilder};
pub use encoders::algorithms::{DecodeError, DictionaryNotFoundError, find_closest_dictionary};
pub use encoders::streaming::{StreamingDecoder, StreamingEncoder};
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
