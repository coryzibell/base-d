mod alphabet;
mod encoding;
mod chunked;
mod config;

pub use alphabet::Alphabet;
pub use config::{AlphabetsConfig, AlphabetConfig, EncodingMode};

use encoding::DecodeError;

pub fn encode(data: &[u8], alphabet: &Alphabet) -> String {
    match alphabet.mode() {
        EncodingMode::BaseConversion => encoding::encode(data, alphabet),
        EncodingMode::Chunked => chunked::encode_chunked(data, alphabet),
    }
}

pub fn decode(encoded: &str, alphabet: &Alphabet) -> Result<Vec<u8>, DecodeError> {
    match alphabet.mode() {
        EncodingMode::BaseConversion => encoding::decode(encoded, alphabet),
        EncodingMode::Chunked => chunked::decode_chunked(encoded, alphabet),
    }
}

#[cfg(test)]
mod tests;
