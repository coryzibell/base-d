//! Alternating word-based encoding for PGP biometric word lists.
//!
//! Unlike standard word encoding which uses radix conversion, this encoder provides
//! a direct 1:1 mapping where each byte is encoded as a single word, with the
//! dictionary selection alternating based on byte position.
//!
//! This is specifically designed for PGP biometric word lists where:
//! - Each byte (0-255) maps to exactly one word
//! - Even byte positions use one dictionary (e.g., "even" words)
//! - Odd byte positions use another dictionary (e.g., "odd" words)
//!
//! # Example
//!
//! ```
//! use base_d::{WordDictionary, AlternatingWordDictionary, word_alternating};
//!
//! // Create dictionaries with 256 words each
//! let even_words: Vec<String> = (0..256).map(|i| format!("even{}", i)).collect();
//! let odd_words: Vec<String> = (0..256).map(|i| format!("odd{}", i)).collect();
//!
//! let even = WordDictionary::builder()
//!     .words(even_words)
//!     .build()
//!     .unwrap();
//!
//! let odd = WordDictionary::builder()
//!     .words(odd_words)
//!     .build()
//!     .unwrap();
//!
//! let dict = AlternatingWordDictionary::new(
//!     vec![even, odd],
//!     "-".to_string(),
//!     false,
//! );
//!
//! let data = vec![0x42, 0xAB];
//! let encoded = word_alternating::encode(&data, &dict);
//! // "even66-odd171" (0x42 = 66, 0xAB = 171)
//!
//! let decoded = word_alternating::decode(&encoded, &dict).unwrap();
//! assert_eq!(decoded, data);
//! ```

use super::errors::DecodeError;
use crate::core::alternating_dictionary::AlternatingWordDictionary;

/// Encodes binary data using alternating word dictionaries.
///
/// Each byte is encoded as a single word, with the dictionary selection
/// alternating based on byte position.
///
/// # Parameters
///
/// - `data`: The binary data to encode
/// - `dictionary`: The alternating word dictionary to use
///
/// # Returns
///
/// A string with words joined by the dictionary's delimiter.
///
/// # Example
///
/// ```
/// use base_d::{WordDictionary, AlternatingWordDictionary, word_alternating};
///
/// let even_words: Vec<String> = (0..256).map(|i| format!("e{}", i)).collect();
/// let odd_words: Vec<String> = (0..256).map(|i| format!("o{}", i)).collect();
///
/// let even = WordDictionary::builder().words(even_words).build().unwrap();
/// let odd = WordDictionary::builder().words(odd_words).build().unwrap();
///
/// let dict = AlternatingWordDictionary::new(vec![even, odd], " ".to_string(), false);
///
/// let data = vec![0x00, 0x01, 0x02];
/// let encoded = word_alternating::encode(&data, &dict);
/// assert_eq!(encoded, "e0 o1 e2");
/// ```
pub fn encode(data: &[u8], dictionary: &AlternatingWordDictionary) -> String {
    if data.is_empty() {
        return String::new();
    }

    let words: Vec<&str> = data
        .iter()
        .enumerate()
        .filter_map(|(pos, &byte)| dictionary.encode_byte(byte, pos))
        .collect();

    words.join(dictionary.delimiter())
}

/// Decodes an alternating word sequence back to binary data.
///
/// Splits the input on the dictionary's delimiter and decodes each word
/// using the appropriate dictionary for that position.
///
/// # Parameters
///
/// - `encoded`: The encoded word sequence
/// - `dictionary`: The alternating word dictionary to use
///
/// # Returns
///
/// The decoded binary data, or a DecodeError if decoding fails.
///
/// # Errors
///
/// Returns `DecodeError::InvalidCharacter` if:
/// - A word is not found in the appropriate dictionary for its position
///
/// # Example
///
/// ```
/// use base_d::{WordDictionary, AlternatingWordDictionary, word_alternating};
///
/// let even_words: Vec<String> = (0..256).map(|i| format!("e{}", i)).collect();
/// let odd_words: Vec<String> = (0..256).map(|i| format!("o{}", i)).collect();
///
/// let even = WordDictionary::builder().words(even_words).build().unwrap();
/// let odd = WordDictionary::builder().words(odd_words).build().unwrap();
///
/// let dict = AlternatingWordDictionary::new(vec![even, odd], " ".to_string(), false);
///
/// let encoded = "e0 o1 e2";
/// let decoded = word_alternating::decode(encoded, &dict).unwrap();
/// assert_eq!(decoded, vec![0x00, 0x01, 0x02]);
/// ```
pub fn decode(
    encoded: &str,
    dictionary: &AlternatingWordDictionary,
) -> Result<Vec<u8>, DecodeError> {
    if encoded.is_empty() {
        return Ok(Vec::new());
    }

    let delimiter = dictionary.delimiter();
    let words: Vec<&str> = if delimiter.is_empty() {
        vec![encoded]
    } else {
        encoded.split(delimiter).collect()
    };

    let mut result = Vec::with_capacity(words.len());

    for (pos, word) in words.iter().enumerate() {
        let byte =
            dictionary
                .decode_word(word.trim(), pos)
                .ok_or_else(|| DecodeError::InvalidWord {
                    word: word.to_string(),
                    position: pos,
                    input: encoded.to_string(),
                })?;
        result.push(byte);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WordDictionary;

    fn create_full_dictionaries() -> AlternatingWordDictionary {
        // Create dictionaries with 256 words each (full byte range)
        let even_words: Vec<String> = (0..256).map(|i| format!("even{}", i)).collect();
        let odd_words: Vec<String> = (0..256).map(|i| format!("odd{}", i)).collect();

        let even = WordDictionary::builder().words(even_words).build().unwrap();

        let odd = WordDictionary::builder().words(odd_words).build().unwrap();

        AlternatingWordDictionary::new(vec![even, odd], "-".to_string(), false)
    }

    fn create_small_dictionaries() -> AlternatingWordDictionary {
        let even = WordDictionary::builder()
            .words(vec!["aardvark", "absurd", "accrue", "acme"])
            .build()
            .unwrap();

        let odd = WordDictionary::builder()
            .words(vec!["adroitness", "adviser", "aftermath", "aggregate"])
            .build()
            .unwrap();

        AlternatingWordDictionary::new(vec![even, odd], "-".to_string(), false)
    }

    #[test]
    fn test_encode_empty() {
        let dict = create_full_dictionaries();
        assert_eq!(encode(&[], &dict), "");
    }

    #[test]
    fn test_encode_single_byte() {
        let dict = create_full_dictionaries();
        let data = vec![0x42];
        let encoded = encode(&data, &dict);
        assert_eq!(encoded, "even66"); // 0x42 = 66
    }

    #[test]
    fn test_encode_two_bytes() {
        let dict = create_full_dictionaries();
        let data = vec![0x42, 0xAB];
        let encoded = encode(&data, &dict);
        assert_eq!(encoded, "even66-odd171"); // 0x42 = 66, 0xAB = 171
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let dict = create_full_dictionaries();
        let data = vec![0x00, 0x01, 0x42, 0xAB, 0xFF];
        let encoded = encode(&data, &dict);
        let decoded = decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decode_empty() {
        let dict = create_full_dictionaries();
        let decoded = decode("", &dict).unwrap();
        assert_eq!(decoded, Vec::<u8>::new());
    }

    #[test]
    fn test_decode_single_word() {
        let dict = create_full_dictionaries();
        let decoded = decode("even66", &dict).unwrap();
        assert_eq!(decoded, vec![0x42]);
    }

    #[test]
    fn test_decode_multiple_words() {
        let dict = create_full_dictionaries();
        let decoded = decode("even66-odd171", &dict).unwrap();
        assert_eq!(decoded, vec![0x42, 0xAB]);
    }

    #[test]
    fn test_decode_case_insensitive() {
        let dict = create_small_dictionaries();
        let data = vec![0, 1];
        let encoded = encode(&data, &dict);

        // Should decode regardless of case
        let decoded_upper = decode(&encoded.to_uppercase(), &dict).unwrap();
        let decoded_lower = decode(&encoded.to_lowercase(), &dict).unwrap();
        assert_eq!(decoded_upper, data);
        assert_eq!(decoded_lower, data);
    }

    #[test]
    fn test_decode_unknown_word() {
        let dict = create_full_dictionaries();
        let result = decode("even0-unknown-even2", &dict);
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::InvalidWord { .. })));
    }

    #[test]
    fn test_decode_wrong_dictionary_for_position() {
        let dict = create_small_dictionaries();
        // "adroitness" is an odd word, but position 0 expects even
        let result = decode("adroitness-absurd", &dict);
        assert!(result.is_err());
    }

    #[test]
    fn test_alternating_pattern() {
        let dict = create_small_dictionaries();
        let data = vec![0, 1, 2, 3];
        let encoded = encode(&data, &dict);

        // Position 0 (even): aardvark (0)
        // Position 1 (odd): adviser (1)
        // Position 2 (even): accrue (2)
        // Position 3 (odd): aggregate (3)
        assert_eq!(encoded, "aardvark-adviser-accrue-aggregate");

        let decoded = decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_custom_delimiter() {
        let even = WordDictionary::builder()
            .words((0..256).map(|i| format!("e{}", i)).collect::<Vec<_>>())
            .build()
            .unwrap();

        let odd = WordDictionary::builder()
            .words((0..256).map(|i| format!("o{}", i)).collect::<Vec<_>>())
            .build()
            .unwrap();

        let dict = AlternatingWordDictionary::new(vec![even, odd], " ".to_string(), false);

        let data = vec![0, 1, 2];
        let encoded = encode(&data, &dict);
        assert_eq!(encoded, "e0 o1 e2");

        let decoded = decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_whitespace_handling() {
        let dict = create_small_dictionaries();
        // Decode should trim whitespace from words
        let decoded = decode("  aardvark  -  adviser  ", &dict).unwrap();
        assert_eq!(decoded, vec![0, 1]);
    }

    #[test]
    fn test_encode_all_bytes() {
        let dict = create_full_dictionaries();
        // Test encoding all possible byte values
        let data: Vec<u8> = (0..=255).collect();
        let encoded = encode(&data, &dict);
        let decoded = decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }
}
