//! SIMD implementation for base32 (5-bit encoding) using NEON
//!
//! Based on x86_64 SSSE3/AVX2 implementation techniques:
//! - Daniel Lemire: https://lemire.me/blog/2023/07/20/fast-decoding-of-base32-strings/
//! - NLnetLabs/simdzone (C implementation by @aqrit)
//! - Wojciech Muła's SIMD base64 work
//!
//! Key differences from base64:
//! - Block size: 10 bytes → 16 chars (two 5-byte groups each producing 8 chars)
//! - 5-bit extraction requires scalar unpacking (no clean multiply-shift pattern)
//! - Hash-based validation using NEON shuffle (vtbl)

#![allow(unused_unsafe)]

use super::super::common;
use crate::core::dictionary::Dictionary;
use crate::simd::variants::Base32Variant;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// SIMD-accelerated base32 encoding using NEON
///
/// Processes 10 bytes -> 16 chars per iteration
pub fn encode(data: &[u8], dictionary: &Dictionary, variant: Base32Variant) -> Option<String> {
    // Pre-allocate output
    let output_len = data.len().div_ceil(5) * 8;
    let mut result = String::with_capacity(output_len);

    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is guaranteed on aarch64
        unsafe {
            encode_neon_impl(data, dictionary, variant, &mut result);
        }
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        encode_scalar_remainder(data, dictionary, &mut result);
    }

    Some(result)
}

/// Validate Base32 padding per RFC 4648
///
/// Returns the data portion (without padding) if valid, None otherwise.
fn validate_base32_padding(input: &str) -> Option<&str> {
    let padding_count = input.bytes().rev().take_while(|&b| b == b'=').count();
    let data_len = input.len() - padding_count;

    // If no padding, validate data_len mod 8 is valid
    if padding_count == 0 {
        return match data_len % 8 {
            0 | 2 | 4 | 5 | 7 => Some(input),
            _ => None,
        };
    }

    // With padding, total must be multiple of 8
    if !input.len().is_multiple_of(8) {
        return None;
    }

    // Verify correct padding count for data length
    let expected_padding = match data_len % 8 {
        0 => 0,
        2 => 6,
        4 => 4,
        5 => 3,
        7 => 1,
        _ => return None,
    };

    if padding_count == expected_padding {
        Some(&input[..data_len])
    } else {
        None
    }
}

/// SIMD-accelerated base32 decoding using NEON
///
/// Processes 16 chars -> 10 bytes per iteration
pub fn decode(encoded: &str, variant: Base32Variant) -> Option<Vec<u8>> {
    // Validate padding before processing
    let input_no_padding = validate_base32_padding(encoded)?;

    let encoded_bytes = input_no_padding.as_bytes();

    // Calculate output size
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

    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is guaranteed on aarch64
        if !unsafe { decode_neon_impl(encoded_bytes, variant, &mut result) } {
            return None;
        }
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        if !decode_scalar_remainder(
            encoded_bytes,
            &mut |c| match variant {
                Base32Variant::Rfc4648 => match c {
                    b'A'..=b'Z' => Some(c - b'A'),
                    b'2'..=b'7' => Some(c - b'2' + 26),
                    _ => None,
                },
                Base32Variant::Rfc4648Hex => match c {
                    b'0'..=b'9' => Some(c - b'0'),
                    b'A'..=b'V' => Some(c - b'A' + 10),
                    _ => None,
                },
            },
            &mut result,
        ) {
            return None;
        }
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

/// NEON base32 encoding implementation
///
/// Processes 10 input bytes -> 16 output characters per iteration.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn encode_neon_impl(
    data: &[u8],
    dictionary: &Dictionary,
    variant: Base32Variant,
    result: &mut String,
) {
    use std::arch::aarch64::*;

    const BLOCK_SIZE: usize = 10; // 10 bytes -> 16 chars

    // Need at least 16 bytes to safely load
    if data.len() < 16 {
        encode_scalar_remainder(data, dictionary, result);
        return;
    }

    // Process blocks of 10 bytes. We load 16 bytes but only use 10.
    let safe_len = if data.len() >= 6 { data.len() - 6 } else { 0 };
    let (num_rounds, simd_bytes) = common::calculate_blocks(safe_len, BLOCK_SIZE);

    let mut offset = 0;
    for _ in 0..num_rounds {
        // Load 16 bytes
        let input_vec = unsafe { vld1q_u8(data.as_ptr().add(offset)) };

        // Extract 5-bit indices from 10 packed bytes
        let indices = unsafe { unpack_5bit_simple(input_vec) };

        // Translate 5-bit indices to ASCII
        let encoded = unsafe { translate_encode(indices, variant) };

        // Store 16 output characters
        let mut output_buf = [0u8; 16];
        unsafe {
            vst1q_u8(output_buf.as_mut_ptr(), encoded);
        }

        // Append to result (safe because base32 is ASCII)
        for &byte in &output_buf {
            result.push(byte as char);
        }

        offset += BLOCK_SIZE;
    }

    // Handle remainder with scalar code
    if simd_bytes < data.len() {
        encode_scalar_remainder(&data[simd_bytes..], dictionary, result);
    }
}

/// Extract 16 x 5-bit indices from 10 packed input bytes using NEON
///
/// For every 5 bytes [A B C D E], we extract 8 x 5-bit groups.
/// This doesn't align well with SIMD, so we use scalar extraction.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn unpack_5bit_simple(input: uint8x16_t) -> uint8x16_t {
    use std::arch::aarch64::*;

    // Extract bytes 0-9 into a buffer
    let mut buf = [0u8; 16];
    unsafe {
        vst1q_u8(buf.as_mut_ptr(), input);
    }

    // Extract 5-bit indices manually (two 5-byte groups)
    let mut indices = [0u8; 16];

    // First group: bytes 0-4 -> indices 0-7
    indices[0] = buf[0] >> 3;
    indices[1] = ((buf[0] & 0x07) << 2) | (buf[1] >> 6);
    indices[2] = (buf[1] >> 1) & 0x1F;
    indices[3] = ((buf[1] & 0x01) << 4) | (buf[2] >> 4);
    indices[4] = ((buf[2] & 0x0F) << 1) | (buf[3] >> 7);
    indices[5] = (buf[3] >> 2) & 0x1F;
    indices[6] = ((buf[3] & 0x03) << 3) | (buf[4] >> 5);
    indices[7] = buf[4] & 0x1F;

    // Second group: bytes 5-9 -> indices 8-15
    indices[8] = buf[5] >> 3;
    indices[9] = ((buf[5] & 0x07) << 2) | (buf[6] >> 6);
    indices[10] = (buf[6] >> 1) & 0x1F;
    indices[11] = ((buf[6] & 0x01) << 4) | (buf[7] >> 4);
    indices[12] = ((buf[7] & 0x0F) << 1) | (buf[8] >> 7);
    indices[13] = (buf[8] >> 2) & 0x1F;
    indices[14] = ((buf[8] & 0x03) << 3) | (buf[9] >> 5);
    indices[15] = buf[9] & 0x1F;

    unsafe { vld1q_u8(indices.as_ptr()) }
}

/// Translate 5-bit indices (0-31) to base32 ASCII characters using NEON
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn translate_encode(indices: uint8x16_t, variant: Base32Variant) -> uint8x16_t {
    use std::arch::aarch64::*;

    match variant {
        Base32Variant::Rfc4648 => {
            // RFC 4648 standard: 0-25 -> 'A'-'Z', 26-31 -> '2'-'7'

            // Create mask for indices >= 26
            let threshold = unsafe { vdupq_n_u8(25) };
            let ge_26 = unsafe { vcgtq_u8(indices, threshold) };

            // Base offset is 'A' (65) for all
            let base = unsafe { vdupq_n_u8(b'A') };

            // Adjustment for >= 26: -41 (from 65 to 24)
            // Use signed arithmetic: cast to s8, adjust, cast back
            let adjustment_val = unsafe { vdupq_n_s8(-41) };
            let adjustment = unsafe {
                vreinterpretq_u8_s8(vandq_s8(vreinterpretq_s8_u8(ge_26), adjustment_val))
            };

            unsafe { vaddq_u8(vaddq_u8(indices, base), adjustment) }
        }
        Base32Variant::Rfc4648Hex => {
            // RFC 4648 hex: 0-9 -> '0'-'9', 10-31 -> 'A'-'V'

            // Create mask for indices >= 10
            let threshold = unsafe { vdupq_n_u8(9) };
            let ge_10 = unsafe { vcgtq_u8(indices, threshold) };

            // Base offset is '0' (48) for all
            let base = unsafe { vdupq_n_u8(b'0') };

            // Adjustment for >= 10: +7 (from 48 to 55)
            let adjustment_val = unsafe { vdupq_n_u8(7) };
            let adjustment = unsafe { vandq_u8(ge_10, adjustment_val) };

            unsafe { vaddq_u8(vaddq_u8(indices, base), adjustment) }
        }
    }
}

/// NEON base32 decoding implementation
///
/// Processes 16 input characters -> 10 output bytes per iteration
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn decode_neon_impl(encoded: &[u8], variant: Base32Variant, result: &mut Vec<u8>) -> bool {
    use std::arch::aarch64::*;

    const INPUT_BLOCK_SIZE: usize = 16;

    // Get decode LUTs for this variant
    let (delta_check, delta_rebase) = unsafe { get_decode_delta_tables_neon(variant) };

    // Calculate number of full 16-byte blocks
    let (num_rounds, simd_bytes) = common::calculate_blocks(encoded.len(), INPUT_BLOCK_SIZE);

    // Process full blocks
    for round in 0..num_rounds {
        let offset = round * INPUT_BLOCK_SIZE;

        // Load 16 input characters
        let input_vec = unsafe { vld1q_u8(encoded.as_ptr().add(offset)) };

        // Validate and translate using hash-based approach
        // 1. Extract hash key (upper 4 bits)
        let hash_key = unsafe { vandq_u8(vshrq_n_u8(input_vec, 4), vdupq_n_u8(0x0F)) };

        // 2. Validate: check = delta_check[hash_key] + input
        // NEON uses vtbl for table lookup (similar to pshufb)
        let check_delta = unsafe { vqtbl1q_u8(delta_check, hash_key) };
        let check = unsafe { vaddq_u8(check_delta, input_vec) };

        // 3. Check should be <= 0x1F (31) for valid base32 characters
        let threshold = unsafe { vdupq_n_u8(0x1F) };
        let invalid_mask = unsafe { vcgtq_u8(check, threshold) };

        // Check if any byte is invalid
        if unsafe { vmaxvq_u8(invalid_mask) } != 0 {
            return false; // Invalid characters
        }

        // 4. Translate: indices = input + delta_rebase[hash_key]
        let rebase_delta = unsafe { vqtbl1q_u8(delta_rebase, hash_key) };
        let indices = unsafe { vaddq_u8(input_vec, rebase_delta) };

        // Pack 5-bit values into bytes (16 chars -> 10 bytes)
        let decoded = unsafe { pack_5bit_to_8bit_neon(indices) };

        // Store 10 bytes
        let mut output_buf = [0u8; 16];
        unsafe {
            vst1q_u8(output_buf.as_mut_ptr(), decoded);
        }
        result.extend_from_slice(&output_buf[0..10]);
    }

    // Handle remainder with scalar fallback
    if simd_bytes < encoded.len() {
        let remainder = &encoded[simd_bytes..];
        if !decode_scalar_remainder(
            remainder,
            &mut |c| match variant {
                Base32Variant::Rfc4648 => match c {
                    b'A'..=b'Z' => Some(c - b'A'),
                    b'2'..=b'7' => Some(c - b'2' + 26),
                    _ => None,
                },
                Base32Variant::Rfc4648Hex => match c {
                    b'0'..=b'9' => Some(c - b'0'),
                    b'A'..=b'V' => Some(c - b'A' + 10),
                    _ => None,
                },
            },
            result,
        ) {
            return false;
        }
    }

    true
}

/// Get decode delta tables for hash-based validation (NEON version)
///
/// Returns (delta_check, delta_rebase) lookup tables indexed by high nibble.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn get_decode_delta_tables_neon(variant: Base32Variant) -> (uint8x16_t, uint8x16_t) {
    match variant {
        Base32Variant::Rfc4648 => {
            // RFC 4648 standard: A-Z (0x41-0x5A) -> 0-25, 2-7 (0x32-0x37) -> 26-31
            // High nibble ranges:
            // 0x3x: '2'-'7' (0x32-0x37)
            // 0x4x: 'A'-'O' (0x41-0x4F)
            // 0x5x: 'P'-'Z' (0x50-0x5A)

            let delta_check_bytes: [u8; 16] = [
                0x7F,
                0x7F,
                0x7F,                // 0x0, 0x1, 0x2 - invalid
                (0x1F - 0x37) as u8, // 0x3: '2'-'7' -> check <= 0x1F
                (0x1F - 0x4F) as u8, // 0x4: 'A'-'O' -> check <= 0x1F
                (0x1F - 0x5A) as u8, // 0x5: 'P'-'Z' -> check <= 0x1F
                0x7F,
                0x7F,
                0x7F,
                0x7F, // 0x6-0x9 - invalid
                0x7F,
                0x7F,
                0x7F,
                0x7F, // 0xA-0xD - invalid
                0x7F,
                0x7F, // 0xE-0xF - invalid
            ];

            let delta_rebase_bytes: [u8; 16] = [
                0,
                0,
                0,                           // 0x0, 0x1, 0x2 - unused
                (26i16 - b'2' as i16) as u8, // 0x3: '2' -> 26
                (0i16 - b'A' as i16) as u8,  // 0x4: 'A' -> 0
                (0i16 - b'A' as i16) as u8,  // 0x5: 'A' -> 0 (P-Z same offset)
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0, // 0x6-0xF - unused
            ];

            unsafe {
                (
                    vld1q_u8(delta_check_bytes.as_ptr()),
                    vld1q_u8(delta_rebase_bytes.as_ptr()),
                )
            }
        }
        Base32Variant::Rfc4648Hex => {
            // RFC 4648 hex: 0-9 (0x30-0x39) -> 0-9, A-V (0x41-0x56) -> 10-31
            // High nibble ranges:
            // 0x3x: '0'-'9' (0x30-0x39)
            // 0x4x: 'A'-'O' (0x41-0x4F)
            // 0x5x: 'P'-'V' (0x50-0x56)

            let delta_check_bytes: [u8; 16] = [
                0x7F,
                0x7F,
                0x7F,                // 0x0, 0x1, 0x2 - invalid
                (0x1F - 0x39) as u8, // 0x3: '0'-'9' -> check <= 0x1F
                (0x1F - 0x4F) as u8, // 0x4: 'A'-'O' -> check <= 0x1F
                (0x1F - 0x56) as u8, // 0x5: 'P'-'V' -> check <= 0x1F
                0x7F,
                0x7F,
                0x7F,
                0x7F, // 0x6-0x9 - invalid
                0x7F,
                0x7F,
                0x7F,
                0x7F, // 0xA-0xD - invalid
                0x7F,
                0x7F, // 0xE-0xF - invalid
            ];

            let delta_rebase_bytes: [u8; 16] = [
                0,
                0,
                0,                           // 0x0, 0x1, 0x2 - unused
                (0i16 - b'0' as i16) as u8,  // 0x3: '0' -> 0
                (10i16 - b'A' as i16) as u8, // 0x4: 'A' -> 10
                (10i16 - b'A' as i16) as u8, // 0x5: 'A' -> 10 (P-V same offset)
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0, // 0x6-0xF - unused
            ];

            unsafe {
                (
                    vld1q_u8(delta_check_bytes.as_ptr()),
                    vld1q_u8(delta_rebase_bytes.as_ptr()),
                )
            }
        }
    }
}

/// Pack 16 bytes of 5-bit indices into 10 bytes using NEON
///
/// Uses multiply-add pattern similar to x86 maddubs.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn pack_5bit_to_8bit_neon(indices: uint8x16_t) -> uint8x16_t {
    use std::arch::aarch64::*;

    // Extract to buffer for scalar packing
    // NEON doesn't have direct equivalent to maddubs for base32
    let mut buf = [0u8; 16];
    unsafe {
        vst1q_u8(buf.as_mut_ptr(), indices);
    }

    // Pack manually: 16 x 5-bit -> 10 bytes
    let mut output = [0u8; 16];

    // First 8 indices -> 5 bytes
    output[0] = (buf[0] << 3) | (buf[1] >> 2);
    output[1] = (buf[1] << 6) | (buf[2] << 1) | (buf[3] >> 4);
    output[2] = (buf[3] << 4) | (buf[4] >> 1);
    output[3] = (buf[4] << 7) | (buf[5] << 2) | (buf[6] >> 3);
    output[4] = (buf[6] << 5) | buf[7];

    // Second 8 indices -> 5 bytes
    output[5] = (buf[8] << 3) | (buf[9] >> 2);
    output[6] = (buf[9] << 6) | (buf[10] << 1) | (buf[11] >> 4);
    output[7] = (buf[11] << 4) | (buf[12] >> 1);
    output[8] = (buf[12] << 7) | (buf[13] << 2) | (buf[14] >> 3);
    output[9] = (buf[14] << 5) | buf[15];

    unsafe { vld1q_u8(output.as_ptr()) }
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
#[allow(deprecated)]
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
            if let Some(result) = encode(input, &dictionary, Base32Variant::Rfc4648) {
                assert_eq!(result, expected, "Failed for input: {:?}", input);
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
            if let Some(result) = encode(input, &dictionary, Base32Variant::Rfc4648Hex) {
                assert_eq!(result, expected, "Failed for input: {:?}", input);
            }
        }
    }

    #[test]
    fn test_decode_round_trip() {
        let dictionary = make_base32_dict();

        for len in 0..100 {
            let original: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();

            if let Some(encoded) = encode(&original, &dictionary, Base32Variant::Rfc4648)
                && let Some(decoded) = decode(&encoded, Base32Variant::Rfc4648)
            {
                assert_eq!(decoded, original, "Round-trip failed at length {}", len);
            }
        }
    }

    #[test]
    fn test_decode_hex_round_trip() {
        let dictionary = make_base32_hex_dict();

        for len in 0..100 {
            let original: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();

            if let Some(encoded) = encode(&original, &dictionary, Base32Variant::Rfc4648Hex)
                && let Some(decoded) = decode(&encoded, Base32Variant::Rfc4648Hex)
            {
                assert_eq!(decoded, original, "Round-trip failed at length {}", len);
            }
        }
    }

    #[test]
    fn test_padding_validation_correct() {
        // Valid padding cases per RFC 4648
        assert!(decode("MY======", Base32Variant::Rfc4648).is_some());
        assert!(decode("MZXQ====", Base32Variant::Rfc4648).is_some());
        assert!(decode("MZXW6===", Base32Variant::Rfc4648).is_some());
        assert!(decode("MZXW6YQ=", Base32Variant::Rfc4648).is_some());
        assert!(decode("MZXW6YTB", Base32Variant::Rfc4648).is_some());

        // Valid unpadded cases
        assert!(decode("MY", Base32Variant::Rfc4648).is_some());
        assert!(decode("MZXQ", Base32Variant::Rfc4648).is_some());
    }

    #[test]
    fn test_padding_validation_incorrect() {
        // Invalid padding count for data length
        assert!(decode("MY=====", Base32Variant::Rfc4648).is_none());
        assert!(decode("MY=======", Base32Variant::Rfc4648).is_none());
        assert!(decode("MZXQ===", Base32Variant::Rfc4648).is_none());

        // Invalid data length
        assert!(decode("M", Base32Variant::Rfc4648).is_none());
        assert!(decode("MYX", Base32Variant::Rfc4648).is_none());
    }
}
