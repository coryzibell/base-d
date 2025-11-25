//! Alphabet variants for SIMD base64 encoding
//!
//! This module defines different base64 alphabet variants and provides
//! utilities to identify which variant a Dictionary uses.

use crate::core::dictionary::Dictionary;

/// Base64 alphabet variants
///
/// Different base64 standards use different characters at positions 62 and 63:
/// - Standard (RFC 4648): uses '+' and '/'
/// - URL-safe (RFC 4648): uses '-' and '_'
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphabetVariant {
    /// Standard base64 alphabet: A-Za-z0-9+/
    Base64Standard,
    /// URL-safe base64 alphabet: A-Za-z0-9-_
    Base64Url,
}

/// Identify which base64 alphabet variant a Dictionary uses
///
/// Returns `None` if the dictionary is not base64 (base != 64) or
/// if it doesn't match a known variant.
///
/// # Arguments
///
/// * `dict` - The Dictionary to identify
///
/// # Returns
///
/// - `Some(AlphabetVariant::Base64Standard)` if positions 62-63 are '+' and '/'
/// - `Some(AlphabetVariant::Base64Url)` if positions 62-63 are '-' and '_'
/// - `None` if not base64 or doesn't match a known variant
pub fn identify_base64_variant(dict: &Dictionary) -> Option<AlphabetVariant> {
    // Only works for base64
    if dict.base() != 64 {
        return None;
    }

    // Check characters at positions 62 and 63
    let char_62 = dict.encode_digit(62)?;
    let char_63 = dict.encode_digit(63)?;

    match (char_62, char_63) {
        ('+', '/') => Some(AlphabetVariant::Base64Standard),
        ('-', '_') => Some(AlphabetVariant::Base64Url),
        _ => None,
    }
}

/// Verify that a dictionary matches the expected base64 alphabet variant
///
/// Checks all 64 positions to ensure they match the expected alphabet.
///
/// # Arguments
///
/// * `dict` - The Dictionary to verify
/// * `variant` - The expected alphabet variant
///
/// # Returns
///
/// `true` if all positions match, `false` otherwise
#[allow(dead_code)]
pub fn verify_base64_alphabet(dict: &Dictionary, variant: AlphabetVariant) -> bool {
    if dict.base() != 64 {
        return false;
    }

    let expected = match variant {
        AlphabetVariant::Base64Standard => {
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        }
        AlphabetVariant::Base64Url => {
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
        }
    };

    for (i, expected_char) in expected.chars().enumerate() {
        if dict.encode_digit(i) != Some(expected_char) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::EncodingMode;

    fn make_base64_standard_dict() -> Dictionary {
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
            .chars()
            .collect();
        Dictionary::new_with_mode(chars, EncodingMode::Chunked, Some('=')).unwrap()
    }

    fn make_base64_url_dict() -> Dictionary {
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
            .chars()
            .collect();
        Dictionary::new_with_mode(chars, EncodingMode::Chunked, Some('=')).unwrap()
    }

    #[test]
    fn test_identify_standard_base64() {
        let dict = make_base64_standard_dict();
        assert_eq!(
            identify_base64_variant(&dict),
            Some(AlphabetVariant::Base64Standard)
        );
    }

    #[test]
    fn test_identify_base64_url() {
        let dict = make_base64_url_dict();
        assert_eq!(
            identify_base64_variant(&dict),
            Some(AlphabetVariant::Base64Url)
        );
    }

    #[test]
    fn test_identify_non_base64() {
        let chars: Vec<char> = "0123456789ABCDEF".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        assert_eq!(identify_base64_variant(&dict), None);
    }

    #[test]
    fn test_identify_unknown_variant() {
        // Custom base64 with different chars at positions 62-63
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@$"
            .chars()
            .collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();
        assert_eq!(identify_base64_variant(&dict), None);
    }

    #[test]
    fn test_verify_standard_alphabet() {
        let dict = make_base64_standard_dict();
        assert!(verify_base64_alphabet(
            &dict,
            AlphabetVariant::Base64Standard
        ));
        assert!(!verify_base64_alphabet(&dict, AlphabetVariant::Base64Url));
    }

    #[test]
    fn test_verify_url_alphabet() {
        let dict = make_base64_url_dict();
        assert!(verify_base64_alphabet(&dict, AlphabetVariant::Base64Url));
        assert!(!verify_base64_alphabet(
            &dict,
            AlphabetVariant::Base64Standard
        ));
    }
}
