mod alphabet;
mod encoding;
mod chunked;
mod byte_range;
mod config;
mod streaming;

pub use alphabet::Alphabet;
pub use config::{AlphabetsConfig, AlphabetConfig, EncodingMode};
pub use streaming::{StreamingEncoder, StreamingDecoder};

use encoding::DecodeError;

pub fn encode(data: &[u8], alphabet: &Alphabet) -> String {
    match alphabet.mode() {
        EncodingMode::BaseConversion => encoding::encode(data, alphabet),
        EncodingMode::Chunked => chunked::encode_chunked(data, alphabet),
        EncodingMode::ByteRange => byte_range::encode_byte_range(data, alphabet),
    }
}

pub fn decode(encoded: &str, alphabet: &Alphabet) -> Result<Vec<u8>, DecodeError> {
    match alphabet.mode() {
        EncodingMode::BaseConversion => encoding::decode(encoded, alphabet),
        EncodingMode::Chunked => chunked::decode_chunked(encoded, alphabet),
        EncodingMode::ByteRange => byte_range::decode_byte_range(encoded, alphabet),
    }
}

#[cfg(test)]
mod tests;
