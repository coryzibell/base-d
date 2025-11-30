//! SIMD implementation for base16/hex (4-bit encoding) using NEON
//!
//! Based on techniques from:
//! - Daniel Lemire's base16 SIMD work
//! - https://lemire.me/blog/2023/07/27/decoding-base16-sequences-quickly/
//!
//! Base16 is simpler than base64 since 4-bit aligns nicely with bytes:
//! - 1 byte = 2 hex chars
//! - 16 bytes = 32 chars (perfect for 128-bit NEON)

#![allow(unused_unsafe)]

use super::super::common;
use crate::core::dictionary::Dictionary;

/// Hex dictionary variant (uppercase vs lowercase)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexVariant {
    /// Uppercase: 0-9A-F
    Uppercase,
    /// Lowercase: 0-9a-f
    Lowercase,
}

/// SIMD-accelerated base16 encoding with NEON
///
/// Processes 16 bytes -> 32 chars per iteration.
/// Falls back to scalar for remainder.
pub fn encode(data: &[u8], _dictionary: &Dictionary, variant: HexVariant) -> Option<String> {
    // Pre-allocate output (2 chars per byte)
    let output_len = data.len() * 2;
    let mut result = String::with_capacity(output_len);

    // SAFETY: Runtime detection verifies CPU feature support
    #[cfg(target_arch = "aarch64")]
    unsafe {
        encode_neon_impl(data, variant, &mut result);
    }

    Some(result)
}

/// SIMD-accelerated base16 decoding with NEON
///
/// Processes 32 chars -> 16 bytes per iteration.
/// Falls back to scalar for remainder.
pub fn decode(encoded: &str, _variant: HexVariant) -> Option<Vec<u8>> {
    let encoded_bytes = encoded.as_bytes();

    // Hex must have even number of chars
    if !encoded_bytes.len().is_multiple_of(2) {
        return None;
    }

    let output_len = encoded_bytes.len() / 2;
    let mut result = Vec::with_capacity(output_len);

    // SAFETY: Runtime detection verifies CPU feature support
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if !decode_neon_impl(encoded_bytes, &mut result) {
            return None;
        }
    }

    Some(result)
}

/// NEON base16 encoding implementation
///
/// Algorithm:
/// 1. Load 16 bytes
/// 2. Extract high nibbles: (byte >> 4) & 0x0F
/// 3. Extract low nibbles: byte & 0x0F
/// 4. Translate nibbles (0-15) to hex ASCII via LUT (vqtbl1q_u8)
/// 5. Interleave high/low: hi[0], lo[0], hi[1], lo[1]... using vzip1q_u8/vzip2q_u8
/// 6. Store 32 chars
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn encode_neon_impl(data: &[u8], variant: HexVariant, result: &mut String) {
    use std::arch::aarch64::*;

    const BLOCK_SIZE: usize = 16;

    if data.len() < BLOCK_SIZE {
        // Fall back to scalar for small inputs
        encode_scalar_remainder(data, variant, result);
        return;
    }

    let (num_rounds, simd_bytes) = common::calculate_blocks(data.len(), BLOCK_SIZE);

    // Build NEON lookup table for nibble -> ASCII translation
    let lut = match variant {
        HexVariant::Uppercase => unsafe {
            vld1q_u8(
                [
                    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'A', b'B', b'C',
                    b'D', b'E', b'F',
                ]
                .as_ptr(),
            )
        },
        HexVariant::Lowercase => unsafe {
            vld1q_u8(
                [
                    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'a', b'b', b'c',
                    b'd', b'e', b'f',
                ]
                .as_ptr(),
            )
        },
    };

    let mask_0f = unsafe { vdupq_n_u8(0x0F) };

    let mut offset = 0;
    for _ in 0..num_rounds {
        let output_buf = unsafe {
            // Load 16 bytes
            let input_vec = vld1q_u8(data.as_ptr().add(offset));

            // Extract high nibbles (shift right by 4)
            let hi_nibbles = vandq_u8(vshrq_n_u8(input_vec, 4), mask_0f);

            // Extract low nibbles
            let lo_nibbles = vandq_u8(input_vec, mask_0f);

            // Translate nibbles to ASCII using table lookup
            let hi_ascii = vqtbl1q_u8(lut, hi_nibbles);
            let lo_ascii = vqtbl1q_u8(lut, lo_nibbles);

            // Interleave high and low bytes: hi[0], lo[0], hi[1], lo[1], ...
            let result_lo = vzip1q_u8(hi_ascii, lo_ascii);
            let result_hi = vzip2q_u8(hi_ascii, lo_ascii);

            // Store 32 output characters
            let mut output_buf = [0u8; 32];
            vst1q_u8(output_buf.as_mut_ptr(), result_lo);
            vst1q_u8(output_buf.as_mut_ptr().add(16), result_hi);
            output_buf
        };

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

/// NEON base16 decoding implementation
///
/// Algorithm:
/// 1. Load 32 hex chars
/// 2. Deinterleave to get high/low nibble streams
/// 3. Validate: check each char is 0-9, A-F, or a-f
/// 4. Translate ASCII to nibble values (0-15)
/// 5. Pack pairs: (high << 4) | low
/// 6. Store 16 bytes
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn decode_neon_impl(encoded: &[u8], result: &mut Vec<u8>) -> bool {
    use std::arch::aarch64::*;

    const INPUT_BLOCK_SIZE: usize = 32;
    const OUTPUT_BLOCK_SIZE: usize = 16;

    let (num_rounds, simd_bytes) = common::calculate_blocks(encoded.len(), INPUT_BLOCK_SIZE);

    // Process full blocks
    for round in 0..num_rounds {
        let offset = round * INPUT_BLOCK_SIZE;

        let (is_valid, output_buf) = unsafe {
            // Load 32 bytes (16 pairs of hex chars)
            let input_lo = vld1q_u8(encoded.as_ptr().add(offset));
            let input_hi = vld1q_u8(encoded.as_ptr().add(offset + 16));

            // Deinterleave: separate high and low nibble chars
            let hi_chars = vuzp1q_u8(input_lo, input_hi);
            let lo_chars = vuzp2q_u8(input_lo, input_hi);

            // Decode both nibble streams
            let hi_vals = decode_nibble_chars_neon(hi_chars);
            let lo_vals = decode_nibble_chars_neon(lo_chars);

            // Check for invalid characters (-1 / 0xFF in decoded values)
            let hi_valid = vmaxvq_u8(hi_vals) < 16;
            let lo_valid = vmaxvq_u8(lo_vals) < 16;

            if !hi_valid || !lo_valid {
                (false, [0u8; 16])
            } else {
                // Pack nibbles into bytes: (high << 4) | low
                let packed = vorrq_u8(vshlq_n_u8(hi_vals, 4), lo_vals);

                // Store 16 bytes
                let mut output_buf = [0u8; 16];
                vst1q_u8(output_buf.as_mut_ptr(), packed);
                (true, output_buf)
            }
        };

        if !is_valid {
            return false;
        }

        result.extend_from_slice(&output_buf[0..OUTPUT_BLOCK_SIZE]);
    }

    // Handle remainder with scalar fallback
    if simd_bytes < encoded.len() && !decode_scalar_remainder(&encoded[simd_bytes..], result) {
        return false;
    }

    true
}

/// Decode a 128-bit vector of hex characters to nibble values (0-15)
///
/// Returns 0xFF for invalid characters
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn decode_nibble_chars_neon(
    chars: std::arch::aarch64::uint8x16_t,
) -> std::arch::aarch64::uint8x16_t {
    use std::arch::aarch64::*;

    // Strategy: Use character ranges to select appropriate lookup
    // '0'-'9': 0x30-0x39 → subtract 0x30 → 0-9
    // 'A'-'F': 0x41-0x46 → subtract 0x37 → 10-15
    // 'a'-'f': 0x61-0x66 → subtract 0x57 → 10-15

    unsafe {
        // Check if char is a digit ('0'-'9': 0x30-0x39)
        let is_digit = vandq_u8(
            vcgtq_u8(chars, vdupq_n_u8(0x2F)), // > '/'
            vcgeq_u8(vdupq_n_u8(0x3A), chars), // <= '9'
        );

        // Check if char is uppercase hex ('A'-'F': 0x41-0x46)
        let is_upper = vandq_u8(
            vcgtq_u8(chars, vdupq_n_u8(0x40)), // > '@'
            vcgeq_u8(vdupq_n_u8(0x47), chars), // <= 'F'
        );

        // Check if char is lowercase hex ('a'-'f': 0x61-0x66)
        let is_lower = vandq_u8(
            vcgtq_u8(chars, vdupq_n_u8(0x60)), // > '`'
            vcgeq_u8(vdupq_n_u8(0x67), chars), // <= 'f'
        );

        // Decode using appropriate offset
        let digit_vals = vandq_u8(is_digit, vsubq_u8(chars, vdupq_n_u8(0x30)));
        let upper_vals = vandq_u8(is_upper, vsubq_u8(chars, vdupq_n_u8(0x37)));
        let lower_vals = vandq_u8(is_lower, vsubq_u8(chars, vdupq_n_u8(0x57)));

        // Combine results (only one should be non-zero per byte)
        let valid_vals = vorrq_u8(vorrq_u8(digit_vals, upper_vals), lower_vals);

        // Set invalid chars to 0xFF
        let is_valid = vorrq_u8(vorrq_u8(is_digit, is_upper), is_lower);
        vorrq_u8(
            vandq_u8(is_valid, valid_vals),
            vbicq_u8(vdupq_n_u8(0xFF), is_valid),
        )
    }
}

/// Decode remaining bytes using scalar algorithm
fn decode_scalar_remainder(data: &[u8], result: &mut Vec<u8>) -> bool {
    if !data.len().is_multiple_of(2) {
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
#[allow(deprecated)]
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

            if let Some(encoded) = encode(&original, &dictionary, HexVariant::Uppercase)
                && let Some(decoded) = decode(&encoded, HexVariant::Uppercase)
            {
                assert_eq!(decoded, original, "Round-trip failed at length {}", len);
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
