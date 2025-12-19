//! Built-in word lists for word-based encoding.
//!
//! Currently includes:
//! - BIP-39 English (2048 words)

use crate::core::word_dictionary::WordDictionary;

/// The BIP-39 English word list (2048 words).
///
/// Used for cryptocurrency seed phrases. Each word encodes 11 bits.
pub const BIP39_ENGLISH: &str = include_str!("bip39-english.txt");

/// Creates a WordDictionary from the built-in BIP-39 English word list.
///
/// # Example
///
/// ```
/// use base_d::wordlists::bip39_english;
/// use base_d::word;
///
/// let dict = bip39_english();
/// assert_eq!(dict.base(), 2048);
///
/// let encoded = word::encode(b"hello", &dict);
/// let decoded = word::decode(&encoded, &dict).unwrap();
/// assert_eq!(decoded, b"hello");
/// ```
pub fn bip39_english() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(BIP39_ENGLISH)
        .delimiter(" ")
        .case_sensitive(false)
        .build()
        .expect("BIP-39 English word list should be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bip39_english_word_count() {
        let dict = bip39_english();
        assert_eq!(dict.base(), 2048);
    }

    #[test]
    fn test_bip39_english_first_word() {
        let dict = bip39_english();
        assert_eq!(dict.encode_word(0), Some("abandon"));
    }

    #[test]
    fn test_bip39_english_last_word() {
        let dict = bip39_english();
        assert_eq!(dict.encode_word(2047), Some("zoo"));
    }

    #[test]
    fn test_bip39_english_roundtrip() {
        use crate::encoders::algorithms::word;

        let dict = bip39_english();
        let data = b"The quick brown fox jumps over the lazy dog";
        let encoded = word::encode(data, &dict);
        let decoded = word::decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_bip39_english_case_insensitive() {
        let dict = bip39_english();
        assert_eq!(dict.decode_word("abandon"), Some(0));
        assert_eq!(dict.decode_word("ABANDON"), Some(0));
        assert_eq!(dict.decode_word("Abandon"), Some(0));
    }
}
