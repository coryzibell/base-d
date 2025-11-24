//! # base-d
//!
//! A universal, multi-dictionary encoding library for Rust.
//!
//! Encode binary data using numerous dictionaries including RFC standards, ancient scripts,
//! emoji, playing cards, and more. Supports three encoding modes: mathematical
//! base conversion, RFC 4648 chunked encoding, and direct byte-range mapping.
//!
//! ## Quick Start
//!
//! ```
//! use base_d::{DictionariesConfig, Dictionary, encode, decode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load built-in dictionaries
//! let config = DictionariesConfig::load_default()?;
//! let base64_config = config.get_dictionary("base64").unwrap();
//!
//! // Create dictionary
//! let chars: Vec<char> = base64_config.chars.chars().collect();
//! let padding = base64_config.padding.as_ref().and_then(|s| s.chars().next());
//! let dictionary = Dictionary::new_with_mode(
//!     chars,
//!     base64_config.mode.clone(),
//!     padding
//! )?;
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
//! - **33 Built-in Alphabets**: RFC standards, emoji, ancient scripts, and more
//! - **3 Encoding Modes**: Mathematical, chunked (RFC-compliant), byte-range
//! - **Streaming Support**: Memory-efficient processing for large files
//! - **Custom Alphabets**: Define your own via TOML configuration
//! - **User Configuration**: Load dictionaries from `~/.config/base-d/dictionaries.toml`
//!
//! ## Encoding Modes
//!
//! ### Mathematical Base Conversion
//!
//! Treats data as a large number. Works with any dictionary size.
//!
//! ```
//! use base_d::{Dictionary, EncodingMode, encode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let chars: Vec<char> = "😀😁😂🤣😃😄😅😆".chars().collect();
//! let dictionary = Dictionary::new_with_mode(
//!     chars,
//!     EncodingMode::BaseConversion,
//!     None
//! )?;
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
//! let dictionary = Dictionary::new_with_mode(
//!     chars,
//!     EncodingMode::Chunked,
//!     Some('=')
//! )?;
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
//! let dictionary = Dictionary::new_with_mode_and_range(
//!     Vec::new(),
//!     EncodingMode::ByteRange,
//!     None,
//!     Some(127991)  // U+1F3F7
//! )?;
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
//! use base_d::{DictionariesConfig, StreamingEncoder};
//! use std::fs::File;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = DictionariesConfig::load_default()?;
//! let alphabet_config = config.get_dictionary("base64").unwrap();
//!
//! // ... create dictionary from config
//! # let chars: Vec<char> = alphabet_config.chars.chars().collect();
//! # let padding = alphabet_config.padding.as_ref().and_then(|s| s.chars().next());
//! # let dictionary = base_d::Dictionary::new_with_mode(chars, alphabet_config.mode.clone(), padding)?;
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
mod compression;
mod detection;

pub use core::dictionary::Dictionary;
pub use core::config::{DictionariesConfig, DictionaryConfig, EncodingMode, CompressionConfig, Settings};
pub use encoders::streaming::{StreamingEncoder, StreamingDecoder};
pub use encoders::encoding::DecodeError;
pub use compression::{CompressionAlgorithm, compress, decompress};
pub use detection::{DictionaryDetector, DictionaryMatch, detect_dictionary};

/// Encodes binary data using the specified dictionary.
///
/// Automatically selects the appropriate encoding strategy based on the
/// dictionary's mode (BaseConversion, Chunked, or ByteRange).
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
/// let dictionary = Dictionary::new_with_mode(chars, EncodingMode::BaseConversion, None)?;
/// let encoded = base_d::encode(b"Hi", &dictionary);
/// # Ok(())
/// # }
/// ```
pub fn encode(data: &[u8], dictionary: &Dictionary) -> String {
    match dictionary.mode() {
        EncodingMode::BaseConversion => encoders::encoding::encode(data, dictionary),
        EncodingMode::Chunked => encoders::chunked::encode_chunked(data, dictionary),
        EncodingMode::ByteRange => encoders::byte_range::encode_byte_range(data, dictionary),
    }
}

/// Decodes a string back to binary data using the specified dictionary.
///
/// Automatically selects the appropriate decoding strategy based on the
/// dictionary's mode (BaseConversion, Chunked, or ByteRange).
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
/// let dictionary = Dictionary::new_with_mode(chars, EncodingMode::BaseConversion, None)?;
/// let data = b"Hi";
/// let encoded = encode(data, &dictionary);
/// let decoded = decode(&encoded, &dictionary)?;
/// assert_eq!(data, &decoded[..]);
/// # Ok(())
/// # }
/// ```
pub fn decode(encoded: &str, dictionary: &Dictionary) -> Result<Vec<u8>, DecodeError> {
    match dictionary.mode() {
        EncodingMode::BaseConversion => encoders::encoding::decode(encoded, dictionary),
        EncodingMode::Chunked => encoders::chunked::decode_chunked(encoded, dictionary),
        EncodingMode::ByteRange => encoders::byte_range::decode_byte_range(encoded, dictionary),
    }
}

#[cfg(test)]
mod tests;
