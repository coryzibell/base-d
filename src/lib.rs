//! # base-d
//!
//! A universal, multi-alphabet encoding library for Rust.
//!
//! Encode binary data to 33+ alphabets including RFC standards, ancient scripts,
//! emoji, playing cards, and more. Supports three encoding modes: mathematical
//! base conversion, RFC 4648 chunked encoding, and direct byte-range mapping.
//!
//! ## Quick Start
//!
//! ```
//! use base_d::{AlphabetsConfig, Alphabet, encode, decode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load built-in alphabets
//! let config = AlphabetsConfig::load_default()?;
//! let base64_config = config.get_alphabet("base64").unwrap();
//!
//! // Create alphabet
//! let chars: Vec<char> = base64_config.chars.chars().collect();
//! let padding = base64_config.padding.as_ref().and_then(|s| s.chars().next());
//! let alphabet = Alphabet::new_with_mode(
//!     chars,
//!     base64_config.mode.clone(),
//!     padding
//! )?;
//!
//! // Encode and decode
//! let data = b"Hello, World!";
//! let encoded = encode(data, &alphabet);
//! let decoded = decode(&encoded, &alphabet)?;
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
//! - **User Configuration**: Load alphabets from `~/.config/base-d/alphabets.toml`
//!
//! ## Encoding Modes
//!
//! ### Mathematical Base Conversion
//!
//! Treats data as a large number. Works with any alphabet size.
//!
//! ```
//! use base_d::{Alphabet, EncodingMode, encode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let chars: Vec<char> = "😀😁😂🤣😃😄😅😆".chars().collect();
//! let alphabet = Alphabet::new_with_mode(
//!     chars,
//!     EncodingMode::BaseConversion,
//!     None
//! )?;
//!
//! let encoded = encode(b"Hi", &alphabet);
//! # Ok(())
//! # }
//! ```
//!
//! ### Chunked Mode (RFC 4648)
//!
//! Fixed-size bit groups, compatible with standard base64/base32.
//!
//! ```
//! use base_d::{Alphabet, EncodingMode, encode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
//!     .chars().collect();
//! let alphabet = Alphabet::new_with_mode(
//!     chars,
//!     EncodingMode::Chunked,
//!     Some('=')
//! )?;
//!
//! let encoded = encode(b"Hello", &alphabet);
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
//! use base_d::{Alphabet, EncodingMode, encode};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let alphabet = Alphabet::new_with_mode_and_range(
//!     Vec::new(),
//!     EncodingMode::ByteRange,
//!     None,
//!     Some(127991)  // U+1F3F7
//! )?;
//!
//! let data = b"Hi";
//! let encoded = encode(data, &alphabet);
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
//! use base_d::{AlphabetsConfig, StreamingEncoder};
//! use std::fs::File;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = AlphabetsConfig::load_default()?;
//! let alphabet_config = config.get_alphabet("base64").unwrap();
//!
//! // ... create alphabet from config
//! # let chars: Vec<char> = alphabet_config.chars.chars().collect();
//! # let padding = alphabet_config.padding.as_ref().and_then(|s| s.chars().next());
//! # let alphabet = base_d::Alphabet::new_with_mode(chars, alphabet_config.mode.clone(), padding)?;
//!
//! let mut input = File::open("large_file.bin")?;
//! let output = File::create("encoded.txt")?;
//!
//! let mut encoder = StreamingEncoder::new(&alphabet, output);
//! encoder.encode(&mut input)?;
//! # Ok(())
//! # }
//! ```

mod core;
mod encoders;

pub use core::alphabet::Alphabet;
pub use core::config::{AlphabetsConfig, AlphabetConfig, EncodingMode};
pub use encoders::streaming::{StreamingEncoder, StreamingDecoder};
pub use encoders::encoding::DecodeError;

/// Encodes binary data using the specified alphabet.
///
/// Automatically selects the appropriate encoding strategy based on the
/// alphabet's mode (BaseConversion, Chunked, or ByteRange).
///
/// # Arguments
///
/// * `data` - The binary data to encode
/// * `alphabet` - The alphabet to use for encoding
///
/// # Returns
///
/// A string containing the encoded data
///
/// # Examples
///
/// ```
/// use base_d::{Alphabet, EncodingMode};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let chars: Vec<char> = "01".chars().collect();
/// let alphabet = Alphabet::new_with_mode(chars, EncodingMode::BaseConversion, None)?;
/// let encoded = base_d::encode(b"Hi", &alphabet);
/// # Ok(())
/// # }
/// ```
pub fn encode(data: &[u8], alphabet: &Alphabet) -> String {
    match alphabet.mode() {
        EncodingMode::BaseConversion => encoders::encoding::encode(data, alphabet),
        EncodingMode::Chunked => encoders::chunked::encode_chunked(data, alphabet),
        EncodingMode::ByteRange => encoders::byte_range::encode_byte_range(data, alphabet),
    }
}

/// Decodes a string back to binary data using the specified alphabet.
///
/// Automatically selects the appropriate decoding strategy based on the
/// alphabet's mode (BaseConversion, Chunked, or ByteRange).
///
/// # Arguments
///
/// * `encoded` - The encoded string to decode
/// * `alphabet` - The alphabet used for encoding
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
/// use base_d::{Alphabet, EncodingMode, encode, decode};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let chars: Vec<char> = "01".chars().collect();
/// let alphabet = Alphabet::new_with_mode(chars, EncodingMode::BaseConversion, None)?;
/// let data = b"Hi";
/// let encoded = encode(data, &alphabet);
/// let decoded = decode(&encoded, &alphabet)?;
/// assert_eq!(data, &decoded[..]);
/// # Ok(())
/// # }
/// ```
pub fn decode(encoded: &str, alphabet: &Alphabet) -> Result<Vec<u8>, DecodeError> {
    match alphabet.mode() {
        EncodingMode::BaseConversion => encoders::encoding::decode(encoded, alphabet),
        EncodingMode::Chunked => encoders::chunked::decode_chunked(encoded, alphabet),
        EncodingMode::ByteRange => encoders::byte_range::decode_byte_range(encoded, alphabet),
    }
}

#[cfg(test)]
mod tests;
