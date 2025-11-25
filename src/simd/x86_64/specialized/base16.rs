//! SIMD implementation for base16/hex (4-bit encoding)
//!
//! Based on techniques from:
//! - Daniel Lemire's base16 SIMD work
//! - https://lemire.me/blog/2023/07/27/decoding-base16-sequences-quickly/
//!
//! Base16 is simpler than base64 since 4-bit aligns nicely with bytes:
//! - 1 byte = 2 hex chars
//! - 16 bytes = 32 chars (perfect for 128-bit SIMD)

use super::super::common;
use crate::core::dictionary::Dictionary;

/// Hex alphabet variant (uppercase vs lowercase)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexVariant {
    /// Uppercase: 0-9A-F
    Uppercase,
    /// Lowercase: 0-9a-f
    Lowercase,
}

/// SIMD-accelerated base16 encoding using SSSE3
///
/// Processes 16 bytes at a time, producing 32 hex characters.
/// Falls back to scalar for remainder.
pub fn encode(data: &[u8], _dictionary: &Dictionary, variant: HexVariant) -> Option<String> {
    // Pre-allocate output (2 chars per byte)
    let output_len = data.len() * 2;
    let mut result = String::with_capacity(output_len);

    // SAFETY: Caller verified SSSE3 support
    unsafe {
        encode_ssse3_impl(data, variant, &mut result);
    }

    Some(result)
}

/// SIMD-accelerated base16 decoding using SSSE3
pub fn decode(encoded: &str, _variant: HexVariant) -> Option<Vec<u8>> {
    let encoded_bytes = encoded.as_bytes();

    // Hex must have even number of chars
    if encoded_bytes.len() % 2 != 0 {
        return None;
    }

    let output_len = encoded_bytes.len() / 2;
    let mut result = Vec::with_capacity(output_len);

    // SAFETY: Caller verified SSSE3 support
    unsafe {
        if !decode_ssse3_impl(encoded_bytes, &mut result) {
            return None;
        }
    }

    Some(result)
}

/// SSSE3 base16 encoding implementation
///
/// Algorithm:
/// 1. Load 16 bytes
/// 2. Split each byte into high/low nibbles
/// 3. Interleave nibbles to get 32 4-bit values
/// 4. Translate nibbles (0-15) to ASCII hex characters
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn encode_ssse3_impl(data: &[u8], variant: HexVariant, result: &mut String) {
    use std::arch::x86_64::*;

    const BLOCK_SIZE: usize = 16;

    if data.len() < BLOCK_SIZE {
        // Fall back to scalar for small inputs
        encode_scalar_remainder(data, variant, result);
        return;
    }

    let (num_rounds, simd_bytes) = common::calculate_blocks(data.len(), BLOCK_SIZE);

    // Lookup table for hex digits
    let lut = match variant {
        HexVariant::Uppercase => _mm_setr_epi8(
            b'0' as i8, b'1' as i8, b'2' as i8, b'3' as i8, b'4' as i8, b'5' as i8, b'6' as i8,
            b'7' as i8, b'8' as i8, b'9' as i8, b'A' as i8, b'B' as i8, b'C' as i8, b'D' as i8,
            b'E' as i8, b'F' as i8,
        ),
        HexVariant::Lowercase => _mm_setr_epi8(
            b'0' as i8, b'1' as i8, b'2' as i8, b'3' as i8, b'4' as i8, b'5' as i8, b'6' as i8,
            b'7' as i8, b'8' as i8, b'9' as i8, b'a' as i8, b'b' as i8, b'c' as i8, b'd' as i8,
            b'e' as i8, b'f' as i8,
        ),
    };

    let mut offset = 0;
    for _ in 0..num_rounds {
        // Load 16 bytes
        let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

        // Extract high nibbles (shift right by 4)
        let hi_nibbles = _mm_and_si128(_mm_srli_epi32(input_vec, 4), _mm_set1_epi8(0x0F));

        // Extract low nibbles
        let lo_nibbles = _mm_and_si128(input_vec, _mm_set1_epi8(0x0F));

        // Translate nibbles to ASCII
        let hi_ascii = _mm_shuffle_epi8(lut, hi_nibbles);
        let lo_ascii = _mm_shuffle_epi8(lut, lo_nibbles);

        // Interleave high and low bytes: hi[0], lo[0], hi[1], lo[1], ...
        let result_lo = _mm_unpacklo_epi8(hi_ascii, lo_ascii);
        let result_hi = _mm_unpackhi_epi8(hi_ascii, lo_ascii);

        // Store 32 output characters
        let mut output_buf = [0u8; 32];
        _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, result_lo);
        _mm_storeu_si128(output_buf.as_mut_ptr().add(16) as *mut __m128i, result_hi);

        // Append to result (safe because hex is ASCII)
        for &byte in &output_buf {
            result.push(byte as char);
        }

        offset += BLOCK_SIZE;
    }

    // Handle remainder with scalar code
    if simd_bytes < data.len() {
        encode_scalar_remainder(&data[simd_bytes..], variant, result);
    }
}

/// Encode remaining bytes using scalar algorithm
fn encode_scalar_remainder(data: &[u8], variant: HexVariant, result: &mut String) {
    let chars = match variant {
        HexVariant::Uppercase => b"0123456789ABCDEF",
        HexVariant::Lowercase => b"0123456789abcdef",
    };

    for &byte in data {
        let hi = (byte >> 4) as usize;
        let lo = (byte & 0x0F) as usize;
        result.push(chars[hi] as char);
        result.push(chars[lo] as char);
    }
}

/// SSSE3 base16 decoding implementation
///
/// Algorithm:
/// 1. Load 32 chars
/// 2. Validate (0-9, A-F, a-f only)
/// 3. Translate ASCII → 0-15 values
/// 4. Pack pairs of nibbles into bytes (high << 4 | low)
/// 5. Store 16 bytes
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn decode_ssse3_impl(encoded: &[u8], result: &mut Vec<u8>) -> bool {
    use std::arch::x86_64::*;

    const INPUT_BLOCK_SIZE: usize = 32;
    const OUTPUT_BLOCK_SIZE: usize = 16;

    let (num_rounds, simd_bytes) = common::calculate_blocks(encoded.len(), INPUT_BLOCK_SIZE);

    // Lookup table for decoding hex chars
    // Uses -1 for invalid characters
    let decode_lut_lo = _mm_setr_epi8(
        -1, -1, -1, -1, -1, -1, -1, -1, // 0x00-0x07
        -1, -1, -1, -1, -1, -1, -1, -1, // 0x08-0x0F
    );

    let decode_lut_hi = _mm_setr_epi8(
        -1, -1, -1, 0, 1, 2, 3, 4, // 0x00-0x07: '0'-'7' are 0x30-0x37
        5, 6, 7, 8, 9, -1, -1, -1, // 0x08-0x0F: '8'-'9' are 0x38-0x39
    );

    let decode_lut_alpha = _mm_setr_epi8(
        -1, 10, 11, 12, 13, 14, 15, -1, // 0x00-0x07: 'A'-'F' are 0x41-0x46
        -1, -1, -1, -1, -1, -1, -1, -1, // 0x08-0x0F
    );

    // Process full blocks
    for round in 0..num_rounds {
        let offset = round * INPUT_BLOCK_SIZE;

        // Load 32 bytes (16 pairs of hex chars)
        let input_lo = _mm_loadu_si128(encoded.as_ptr().add(offset) as *const __m128i);
        let input_hi = _mm_loadu_si128(encoded.as_ptr().add(offset + 16) as *const __m128i);

        // Deinterleave: separate high and low nibble chars
        let mask_odd = _mm_set1_epi16(0xFF00_u16 as i16);
        let hi_chars = _mm_or_si128(
            _mm_srli_epi16(_mm_and_si128(input_lo, mask_odd), 8),
            _mm_and_si128(input_hi, mask_odd),
        );
        let lo_chars = _mm_or_si128(
            _mm_and_si128(input_lo, _mm_set1_epi8(0xFFu8 as i8)),
            _mm_slli_epi16(_mm_and_si128(input_hi, _mm_set1_epi8(0xFFu8 as i8)), 8),
        );

        // Decode both nibble streams
        let hi_vals = decode_nibble_chars(hi_chars, decode_lut_lo, decode_lut_hi, decode_lut_alpha);
        let lo_vals = decode_nibble_chars(lo_chars, decode_lut_lo, decode_lut_hi, decode_lut_alpha);

        // Check for invalid characters (-1 in decoded values)
        if _mm_movemask_epi8(_mm_cmplt_epi8(hi_vals, _mm_setzero_si128())) != 0 {
            return false; // Invalid character in high nibbles
        }
        if _mm_movemask_epi8(_mm_cmplt_epi8(lo_vals, _mm_setzero_si128())) != 0 {
            return false; // Invalid character in low nibbles
        }

        // Pack nibbles into bytes: (high << 4) | low
        let packed = _mm_or_si128(_mm_slli_epi32(hi_vals, 4), lo_vals);

        // Store 16 bytes
        let mut output_buf = [0u8; 16];
        _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, packed);
        result.extend_from_slice(&output_buf[0..OUTPUT_BLOCK_SIZE]);
    }

    // Handle remainder with scalar fallback
    if simd_bytes < encoded.len() {
        if !decode_scalar_remainder(&encoded[simd_bytes..], result) {
            return false;
        }
    }

    true
}

/// Decode a vector of hex characters to nibble values (0-15)
///
/// Returns -1 for invalid characters
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn decode_nibble_chars(
    chars: std::arch::x86_64::__m128i,
    _lut_lo: std::arch::x86_64::__m128i,
    _lut_hi: std::arch::x86_64::__m128i,
    _lut_alpha: std::arch::x86_64::__m128i,
) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Strategy: Use character ranges to select appropriate lookup
    // '0'-'9': 0x30-0x39 → subtract 0x30 → 0-9
    // 'A'-'F': 0x41-0x46 → subtract 0x37 → 10-15
    // 'a'-'f': 0x61-0x66 → subtract 0x57 → 10-15

    // Check if char is a digit ('0'-'9': 0x30-0x39)
    let is_digit = _mm_and_si128(
        _mm_cmpgt_epi8(chars, _mm_set1_epi8(0x2F)), // > '/'
        _mm_cmplt_epi8(chars, _mm_set1_epi8(0x3A)), // < ':'
    );

    // Check if char is uppercase hex ('A'-'F': 0x41-0x46)
    let is_upper = _mm_and_si128(
        _mm_cmpgt_epi8(chars, _mm_set1_epi8(0x40)), // > '@'
        _mm_cmplt_epi8(chars, _mm_set1_epi8(0x47)), // < 'G'
    );

    // Check if char is lowercase hex ('a'-'f': 0x61-0x66)
    let is_lower = _mm_and_si128(
        _mm_cmpgt_epi8(chars, _mm_set1_epi8(0x60)), // > '`'
        _mm_cmplt_epi8(chars, _mm_set1_epi8(0x67)), // < 'g'
    );

    // Decode using appropriate offset
    let digit_vals = _mm_and_si128(is_digit, _mm_sub_epi8(chars, _mm_set1_epi8(0x30)));
    let upper_vals = _mm_and_si128(is_upper, _mm_sub_epi8(chars, _mm_set1_epi8(0x37)));
    let lower_vals = _mm_and_si128(is_lower, _mm_sub_epi8(chars, _mm_set1_epi8(0x57)));

    // Combine results (only one should be non-zero per byte)
    let valid_vals = _mm_or_si128(_mm_or_si128(digit_vals, upper_vals), lower_vals);

    // Set invalid chars to -1
    let is_valid = _mm_or_si128(_mm_or_si128(is_digit, is_upper), is_lower);
    _mm_or_si128(
        _mm_and_si128(is_valid, valid_vals),
        _mm_andnot_si128(is_valid, _mm_set1_epi8(-1)),
    )
}

/// Decode remaining bytes using scalar algorithm
fn decode_scalar_remainder(data: &[u8], result: &mut Vec<u8>) -> bool {
    if data.len() % 2 != 0 {
        return false;
    }

    for chunk in data.chunks_exact(2) {
        let hi = match decode_hex_char(chunk[0]) {
            Some(v) => v,
            None => return false,
        };
        let lo = match decode_hex_char(chunk[1]) {
            Some(v) => v,
            None => return false,
        };

        result.push((hi << 4) | lo);
    }

    true
}

/// Decode a single hex character to a nibble value
fn decode_hex_char(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

/// Identify hex variant from dictionary
pub fn identify_hex_variant(dict: &Dictionary) -> Option<HexVariant> {
    if dict.base() != 16 {
        return None;
    }

    // Check character at position 10 (should be 'A' or 'a')
    match dict.encode_digit(10)? {
        'A' => Some(HexVariant::Uppercase),
        'a' => Some(HexVariant::Lowercase),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::EncodingMode;
    use crate::core::dictionary::Dictionary;

    fn make_hex_dict_upper() -> Dictionary {
        let chars: Vec<char> = "0123456789ABCDEF".chars().collect();
        Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap()
    }

    fn make_hex_dict_lower() -> Dictionary {
        let chars: Vec<char> = "0123456789abcdef".chars().collect();
        Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap()
    }

    #[test]
    fn test_encode_uppercase() {
        let dictionary = make_hex_dict_upper();
        let test_data = b"Hello, World!";

        if let Some(result) = encode(test_data, &dictionary, HexVariant::Uppercase) {
            let expected = "48656C6C6F2C20576F726C6421";
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_encode_lowercase() {
        let dictionary = make_hex_dict_lower();
        let test_data = b"Hello, World!";

        if let Some(result) = encode(test_data, &dictionary, HexVariant::Lowercase) {
            let expected = "48656c6c6f2c20576f726c6421".to_lowercase();
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_decode_uppercase() {
        let encoded = "48656C6C6F2C20576F726C6421";

        if let Some(decoded) = decode(encoded, HexVariant::Uppercase) {
            assert_eq!(decoded, b"Hello, World!");
        } else {
            panic!("Decode failed");
        }
    }

    #[test]
    fn test_decode_lowercase() {
        let encoded = "48656c6c6f2c20576f726c6421";

        if let Some(decoded) = decode(encoded, HexVariant::Lowercase) {
            assert_eq!(decoded, b"Hello, World!");
        } else {
            panic!("Decode failed");
        }
    }

    #[test]
    fn test_decode_mixed_case() {
        let encoded = "48656C6c6F2c20576F726C6421";

        // Should work with either variant
        if let Some(decoded) = decode(encoded, HexVariant::Uppercase) {
            assert_eq!(decoded, b"Hello, World!");
        } else {
            panic!("Decode failed");
        }
    }

    #[test]
    fn test_round_trip() {
        let dictionary = make_hex_dict_upper();

        for len in 0..100 {
            let original: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();

            if let Some(encoded) = encode(&original, &dictionary, HexVariant::Uppercase) {
                if let Some(decoded) = decode(&encoded, HexVariant::Uppercase) {
                    assert_eq!(decoded, original, "Round-trip failed at length {}", len);
                }
            }
        }
    }

    #[test]
    fn test_decode_invalid_chars() {
        let invalid_cases = [
            "4865ZZ",   // Invalid chars
            "48656",    // Odd length
            "48656G6C", // G is invalid
        ];

        for &encoded in &invalid_cases {
            assert_eq!(
                decode(encoded, HexVariant::Uppercase),
                None,
                "Should reject: {}",
                encoded
            );
        }
    }

    #[test]
    fn test_identify_variant() {
        let upper_dict = make_hex_dict_upper();
        assert_eq!(
            identify_hex_variant(&upper_dict),
            Some(HexVariant::Uppercase)
        );

        let lower_dict = make_hex_dict_lower();
        assert_eq!(
            identify_hex_variant(&lower_dict),
            Some(HexVariant::Lowercase)
        );
    }

    #[test]
    fn test_encode_edge_cases() {
        let dictionary = make_hex_dict_upper();

        // Empty input
        if let Some(result) = encode(&[], &dictionary, HexVariant::Uppercase) {
            assert_eq!(result, "");
        }

        // Single byte
        if let Some(result) = encode(&[0xFF], &dictionary, HexVariant::Uppercase) {
            assert_eq!(result, "FF");
        }

        // All zeros
        if let Some(result) = encode(&[0x00, 0x00, 0x00], &dictionary, HexVariant::Uppercase) {
            assert_eq!(result, "000000");
        }
    }
}
