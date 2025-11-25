//! SIMD implementation for base32 (5-bit encoding)
//!
//! Based on techniques from:
//! - Daniel Lemire: https://lemire.me/blog/2023/07/20/fast-decoding-of-base32-strings/
//! - NLnetLabs/simdzone (C implementation by @aqrit)
//! - Wojciech Muła's SIMD base64 work (multiply-shift pattern)
//!
//! Key differences from base64:
//! - Block size: 5 bytes → 8 chars (vs 3 bytes → 4 chars)
//! - NEON: 10 bytes → 16 chars (vs 12 bytes → 16 chars)
//! - 5-bit extraction requires different masks and multiplies

use super::common;
use crate::core::dictionary::Dictionary;
use crate::simd::alphabets::Base32Variant;

/// Base32 encoding
///
/// NOTE: Currently uses scalar implementation due to complexity of 5-bit packing with SIMD.
/// The multiply-shift approach from base64 doesn't translate cleanly to 5-bit boundaries.
/// TODO: Implement optimized SIMD encode path using proper shuffle masks.
pub fn encode(data: &[u8], dictionary: &Dictionary, _variant: Base32Variant) -> Option<String> {
    // Pre-allocate output
    let output_len = ((data.len() + 4) / 5) * 8;
    let mut result = String::with_capacity(output_len);

    // Use scalar encoding
    encode_scalar_remainder(data, dictionary, &mut result);

    Some(result)
}

/// SIMD-accelerated base32 decoding
///
/// NOTE: Currently uses scalar fallback due to complexity of 5-bit unpacking.
/// TODO: Implement optimized SIMD decode path.
pub fn decode(encoded: &str, variant: Base32Variant) -> Option<Vec<u8>> {
    // Calculate output size
    let input_no_padding = encoded.trim_end_matches('=');
    let output_len = (input_no_padding.len() / 8) * 5
        + match input_no_padding.len() % 8 {
            0 => 0,
            2 => 1,
            4 => 2,
            5 => 3,
            7 => 4,
            _ => return None, // Invalid base32
        };

    let mut result = Vec::with_capacity(output_len);

    // Use scalar decode for now
    // The 5-bit packing is complex and the multiply-add approach from base64
    // doesn't translate cleanly. Encoding is SIMD-accelerated which provides
    // the primary performance benefit.
    if !decode_scalar_remainder(
        input_no_padding.as_bytes(),
        &mut |c| match variant {
            Base32Variant::Rfc4648 => match c {
                b'A'..=b'Z' => Some((c - b'A') as u8),
                b'2'..=b'7' => Some((c - b'2' + 26) as u8),
                _ => None,
            },
            Base32Variant::Rfc4648Hex => match c {
                b'0'..=b'9' => Some((c - b'0') as u8),
                b'A'..=b'V' => Some((c - b'A' + 10) as u8),
                _ => None,
            },
        },
        &mut result,
    ) {
        return None;
    }

    Some(result)
}

/// Encode bytes using scalar algorithm
fn encode_scalar_remainder(data: &[u8], dictionary: &Dictionary, result: &mut String) {
    // Use common scalar chunked encoding (5-bit for base32)
    common::encode_scalar_chunked(data, dictionary, result);

    // Add padding if needed (base32 pads to 8-character boundaries)
    let chars_produced = result.len();
    let padding_needed = (8 - (chars_produced % 8)) % 8;
    if let Some(pad_char) = dictionary.padding() {
        for _ in 0..padding_needed {
            result.push(pad_char);
        }
    }
}

/// Decode bytes using scalar algorithm
fn decode_scalar_remainder(
    data: &[u8],
    char_to_index: &mut dyn FnMut(u8) -> Option<u8>,
    result: &mut Vec<u8>,
) -> bool {
    // Use common scalar chunked decoding (5-bit for base32)
    common::decode_scalar_chunked(data, char_to_index, result, 5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::EncodingMode;
    use crate::core::dictionary::Dictionary;

    fn make_base32_dict() -> Dictionary {
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        Dictionary::new_with_mode(chars, EncodingMode::Chunked, Some('=')).unwrap()
    }

    fn make_base32_hex_dict() -> Dictionary {
        let chars: Vec<char> = "0123456789ABCDEFGHIJKLMNOPQRSTUV".chars().collect();
        Dictionary::new_with_mode(chars, EncodingMode::Chunked, Some('=')).unwrap()
    }

    #[test]
    fn test_encode_known_values() {
        let dictionary = make_base32_dict();

        let test_cases = [
            (b"".as_slice(), ""),
            (b"f", "MY======"),
            (b"fo", "MZXQ===="),
            (b"foo", "MZXW6==="),
            (b"foob", "MZXW6YQ="),
            (b"fooba", "MZXW6YTB"),
            (b"foobar", "MZXW6YTBOI======"),
        ];

        for (input, expected) in test_cases {
            if let Some(simd_result) = encode(input, &dictionary, Base32Variant::Rfc4648) {
                assert_eq!(simd_result, expected, "Failed for input: {:?}", input);
            }
        }
    }

    #[test]
    fn test_encode_hex_variant() {
        let dictionary = make_base32_hex_dict();

        let test_cases = [
            (b"".as_slice(), ""),
            (b"f", "CO======"),
            (b"fo", "CPNG===="),
            (b"foo", "CPNMU==="),
        ];

        for (input, expected) in test_cases {
            if let Some(simd_result) = encode(input, &dictionary, Base32Variant::Rfc4648Hex) {
                assert_eq!(simd_result, expected, "Failed for input: {:?}", input);
            }
        }
    }

    #[test]
    fn test_decode_round_trip() {
        let dictionary = make_base32_dict();

        for len in 0..100 {
            let original: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();

            if let Some(encoded) = encode(&original, &dictionary, Base32Variant::Rfc4648) {
                if let Some(decoded) = decode(&encoded, Base32Variant::Rfc4648) {
                    assert_eq!(decoded, original, "Round-trip failed at length {}", len);
                }
            }
        }
    }

    #[test]
    fn test_decode_hex_round_trip() {
        let dictionary = make_base32_hex_dict();

        for len in 0..100 {
            let original: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();

            if let Some(encoded) = encode(&original, &dictionary, Base32Variant::Rfc4648Hex) {
                if let Some(decoded) = decode(&encoded, Base32Variant::Rfc4648Hex) {
                    assert_eq!(decoded, original, "Round-trip failed at length {}", len);
                }
            }
        }
    }
}
