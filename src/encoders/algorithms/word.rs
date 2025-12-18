//! Word-based encoding using radix conversion.
//!
//! Same mathematical approach as character-based radix encoding,
//! but outputs words joined by a delimiter instead of concatenated characters.

use crate::core::word_dictionary::WordDictionary;
use num_integer::Integer;
use num_traits::Zero;

pub use super::errors::DecodeError;

/// Encodes binary data as a sequence of words.
///
/// Uses radix (base) conversion where each "digit" is a word from the dictionary.
/// Words are joined by the dictionary's delimiter.
///
/// # Example
///
/// ```
/// use base_d::{WordDictionary, word};
///
/// let dict = WordDictionary::builder()
///     .words(vec!["abandon", "ability", "able", "about"])
///     .delimiter(" ")
///     .build()
///     .unwrap();
///
/// let encoded = word::encode(b"\x00\x01\x02", &dict);
/// // Result is words joined by spaces
/// ```
pub fn encode(data: &[u8], dictionary: &WordDictionary) -> String {
    if data.is_empty() {
        return String::new();
    }

    // Count leading zeros for efficient handling
    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();

    // If all zeros, return early
    if leading_zeros == data.len() {
        let zero_word = dictionary.encode_word(0).unwrap();
        return std::iter::repeat(zero_word)
            .take(data.len())
            .collect::<Vec<_>>()
            .join(dictionary.delimiter());
    }

    let base = dictionary.base();
    let mut num = num_bigint::BigUint::from_bytes_be(&data[leading_zeros..]);

    // Pre-allocate result vector with estimated capacity
    let max_words =
        ((data.len() - leading_zeros) * 8 * 1000) / (base as f64).log2() as usize / 1000 + 1;
    let mut result: Vec<&str> = Vec::with_capacity(max_words + leading_zeros);

    let base_big = num_bigint::BigUint::from(base);

    while !num.is_zero() {
        let (quotient, remainder) = num.div_rem(&base_big);
        let digit = remainder.to_u64_digits();
        let digit_val = if digit.is_empty() {
            0
        } else {
            digit[0] as usize
        };
        result.push(dictionary.encode_word(digit_val).unwrap());
        num = quotient;
    }

    // Add leading zeros
    let zero_word = dictionary.encode_word(0).unwrap();
    for _ in 0..leading_zeros {
        result.push(zero_word);
    }

    result.reverse();
    result.join(dictionary.delimiter())
}

/// Decodes a word sequence back to binary data.
///
/// Splits the input on the dictionary's delimiter, then performs
/// reverse radix conversion.
///
/// # Errors
///
/// Returns `DecodeError` if:
/// - Input is empty
/// - A word is not found in the dictionary
pub fn decode(encoded: &str, dictionary: &WordDictionary) -> Result<Vec<u8>, DecodeError> {
    if encoded.is_empty() {
        return Err(DecodeError::EmptyInput);
    }

    let base = dictionary.base();
    let mut num = num_bigint::BigUint::from(0u8);
    let base_big = num_bigint::BigUint::from(base);

    // Split on delimiter
    let words: Vec<&str> = encoded.split(dictionary.delimiter()).collect();
    let mut leading_zeros = 0;

    // Track position for error reporting
    let mut char_position = 0;
    for word in &words {
        let digit = dictionary.decode_word(word).ok_or_else(|| {
            DecodeError::invalid_word(word, char_position, encoded)
        })?;

        if num.is_zero() && digit == 0 {
            leading_zeros += 1;
        } else {
            num *= &base_big;
            num += num_bigint::BigUint::from(digit);
        }

        // Track position (word + delimiter)
        char_position += word.len() + dictionary.delimiter().len();
    }

    // Handle all-zero case
    if num.is_zero() && leading_zeros > 0 {
        return Ok(vec![0u8; leading_zeros]);
    }

    let bytes = num.to_bytes_be();

    // Construct result with pre-allocated capacity
    let mut result = Vec::with_capacity(leading_zeros + bytes.len());
    result.resize(leading_zeros, 0u8);
    result.extend_from_slice(&bytes);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dictionary() -> WordDictionary {
        // Small dictionary for testing (base 4)
        WordDictionary::builder()
            .words(vec!["zero", "one", "two", "three"])
            .delimiter(" ")
            .build()
            .unwrap()
    }

    fn bip39_style_dictionary() -> WordDictionary {
        // Larger dictionary mimicking BIP-39 structure (base 16 for easier testing)
        WordDictionary::builder()
            .words(vec![
                "abandon", "ability", "able", "about",
                "above", "absent", "absorb", "abstract",
                "absurd", "abuse", "access", "accident",
                "account", "accuse", "achieve", "acid",
            ])
            .delimiter(" ")
            .build()
            .unwrap()
    }

    #[test]
    fn test_encode_empty() {
        let dict = test_dictionary();
        assert_eq!(encode(&[], &dict), "");
    }

    #[test]
    fn test_encode_single_byte() {
        let dict = test_dictionary();
        // 0x05 in base 4 = 1*4 + 1 = "one one"
        let encoded = encode(&[0x05], &dict);
        assert_eq!(encoded, "one one");
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let dict = test_dictionary();
        let data = b"hello";
        let encoded = encode(data, &dict);
        let decoded = decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_encode_decode_roundtrip_larger() {
        let dict = bip39_style_dictionary();
        let data = b"The quick brown fox";
        let encoded = encode(data, &dict);
        let decoded = decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_leading_zeros_preserved() {
        let dict = test_dictionary();
        let data = &[0x00, 0x00, 0x05];
        let encoded = encode(data, &dict);
        let decoded = decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_all_zeros() {
        let dict = test_dictionary();
        let data = &[0x00, 0x00, 0x00];
        let encoded = encode(data, &dict);
        assert_eq!(encoded, "zero zero zero");
        let decoded = decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decode_empty_error() {
        let dict = test_dictionary();
        let result = decode("", &dict);
        assert!(matches!(result, Err(DecodeError::EmptyInput)));
    }

    #[test]
    fn test_decode_unknown_word() {
        let dict = test_dictionary();
        let result = decode("zero unknown one", &dict);
        assert!(result.is_err());
    }

    #[test]
    fn test_case_insensitive_decode() {
        let dict = WordDictionary::builder()
            .words(vec!["Alpha", "Bravo", "Charlie", "Delta"])
            .case_sensitive(false)
            .build()
            .unwrap();

        let data = &[0x01];
        let encoded = encode(data, &dict);

        // Should decode regardless of case
        let decoded_lower = decode(&encoded.to_lowercase(), &dict).unwrap();
        let decoded_upper = decode(&encoded.to_uppercase(), &dict).unwrap();
        assert_eq!(decoded_lower, data);
        assert_eq!(decoded_upper, data);
    }

    #[test]
    fn test_custom_delimiter() {
        let dict = WordDictionary::builder()
            .words(vec!["a", "b", "c", "d"])
            .delimiter("-")
            .build()
            .unwrap();

        let data = &[0x05];
        let encoded = encode(data, &dict);
        assert!(encoded.contains("-"));

        let decoded = decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }
}
