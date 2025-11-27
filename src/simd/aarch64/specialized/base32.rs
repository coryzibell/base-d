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
use crate::simd::variants::Base32Variant;

/// NEON-accelerated base32 encoding
///
/// Processes 10 input bytes -> 16 output characters per iteration.
/// Falls back to scalar for remainder.
pub fn encode(data: &[u8], dictionary: &Dictionary, variant: Base32Variant) -> Option<String> {
    // Pre-allocate output
    let output_len = ((data.len() + 4) / 5) * 8;
    let mut result = String::with_capacity(output_len);

    unsafe {
        encode_neon_impl(data, dictionary, variant, &mut result);
    }

    Some(result)
}

/// NEON-accelerated base32 decoding
///
/// Processes 16 input characters -> 10 output bytes per iteration.
/// Falls back to scalar for remainder.
pub fn decode(encoded: &str, variant: Base32Variant) -> Option<Vec<u8>> {
    let encoded_bytes = encoded.as_bytes();

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

    unsafe {
        if !decode_neon_impl(encoded_bytes, variant, &mut result) {
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

    // Need at least 16 bytes in buffer to safely load 128 bits
    if data.len() < 16 {
        // Fall back to scalar for small inputs
        encode_scalar_remainder(data, dictionary, result);
        return;
    }

    // Process blocks of 10 bytes. We load 16 bytes but only use 10.
    // Ensure we don't read past the buffer: need 6 extra bytes after last block
    let safe_len = if data.len() >= 6 { data.len() - 6 } else { 0 };
    let num_blocks = safe_len / BLOCK_SIZE;
    let simd_bytes = num_blocks * BLOCK_SIZE;

    let mut offset = 0;
    for _ in 0..num_blocks {
        // Load 16 bytes (we only use the first 10)
        let input_vec = vld1q_u8(data.as_ptr().add(offset));

        // Extract 5-bit indices from 10 packed bytes
        let indices = unpack_5bit_simple_neon(input_vec);

        // Translate 5-bit indices to ASCII
        let encoded = translate_encode_neon(indices, variant);

        // Store 16 output characters
        let mut output_buf = [0u8; 16];
        vst1q_u8(output_buf.as_mut_ptr(), encoded);

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

/// Simple 5-bit unpacking using direct shifts and masks (NEON)
///
/// Extracts 16 x 5-bit values from 10 bytes
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn unpack_5bit_simple_neon(
    input: std::arch::aarch64::uint8x16_t,
) -> std::arch::aarch64::uint8x16_t {
    use std::arch::aarch64::*;

    // Extract bytes 0-9 into a buffer for easier manipulation
    let mut buf = [0u8; 16];
    vst1q_u8(buf.as_mut_ptr(), input);

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

    vld1q_u8(indices.as_ptr())
}

/// Translate 5-bit indices (0-31) to base32 ASCII characters (NEON)
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn translate_encode_neon(
    indices: std::arch::aarch64::uint8x16_t,
    variant: Base32Variant,
) -> std::arch::aarch64::uint8x16_t {
    use std::arch::aarch64::*;

    match variant {
        Base32Variant::Rfc4648 => {
            // RFC 4648 standard: 0-25 -> 'A'-'Z', 26-31 -> '2'-'7'
            // Create mask for indices >= 26
            let indices_signed = vreinterpretq_s8_u8(indices);
            let ge_26 = vcgtq_s8(indices_signed, vdupq_n_s8(25));

            // Base offset is 'A' (65) for all
            let base = vdupq_n_u8(b'A');

            // Adjustment for >= 26: we want '2' (50) for index 26
            // So offset should be 50 - 26 = 24 instead of 65
            // Difference: 24 - 65 = -41
            let adjustment = vandq_u8(ge_26, vdupq_n_u8((-41i8) as u8));

            vaddq_u8(vaddq_u8(indices, base), adjustment)
        }
        Base32Variant::Rfc4648Hex => {
            // RFC 4648 hex: 0-9 -> '0'-'9', 10-31 -> 'A'-'V'
            // Create mask for indices >= 10
            let indices_signed = vreinterpretq_s8_u8(indices);
            let ge_10 = vcgtq_s8(indices_signed, vdupq_n_s8(9));

            // Base offset is '0' (48) for indices 0-9
            let base = vdupq_n_u8(b'0');

            // Adjustment for >= 10: we want 'A' (65) for index 10
            // So offset should be 65 - 10 = 55 instead of 48
            // Difference: 55 - 48 = 7
            let adjustment = vandq_u8(ge_10, vdupq_n_u8(7));

            vaddq_u8(vaddq_u8(indices, base), adjustment)
        }
    }
}

/// NEON base32 decoding implementation
///
/// Based on Lemire's algorithm: https://lemire.me/blog/2023/07/20/fast-decoding-of-base32-strings/
/// Processes 16 input characters -> 10 output bytes per iteration
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn decode_neon_impl(encoded: &[u8], variant: Base32Variant, result: &mut Vec<u8>) -> bool {
    use std::arch::aarch64::*;

    const INPUT_BLOCK_SIZE: usize = 16;

    // Strip padding
    let input_no_padding = if let Some(last_non_pad) = encoded.iter().rposition(|&b| b != b'=') {
        &encoded[..=last_non_pad]
    } else {
        encoded
    };

    // Get decode LUTs for this variant
    let (delta_check, delta_rebase) = get_decode_delta_tables_neon(variant);

    // Calculate number of full 16-byte blocks
    let num_blocks = input_no_padding.len() / INPUT_BLOCK_SIZE;
    let simd_bytes = num_blocks * INPUT_BLOCK_SIZE;

    // Process full blocks
    for round in 0..num_blocks {
        let offset = round * INPUT_BLOCK_SIZE;

        // Load 16 bytes
        let input_vec = vld1q_u8(input_no_padding.as_ptr().add(offset));

        // Validate and translate using hash-based approach
        // 1. Extract hash key (upper 4 bits)
        let input_u32 = vreinterpretq_u32_u8(input_vec);
        let hash_key_u32 = vandq_u32(vshrq_n_u32(input_u32, 4), vdupq_n_u32(0x0F0F0F0F));
        let hash_key = vreinterpretq_u8_u32(hash_key_u32);

        // 2. Validate: check = delta_check[hash_key] + input
        let check = vaddq_u8(vqtbl1q_u8(delta_check, hash_key), input_vec);

        // 3. Check should be <= 0x1F (31) for valid base32 characters
        let check_signed = vreinterpretq_s8_u8(check);
        let invalid_mask = vcgtq_s8(check_signed, vdupq_n_s8(0x1F));

        // Check if any byte is invalid (use vmaxvq_u8 to test if any bit set)
        // vcgtq_s8 returns uint8x16_t, already the correct type
        if vmaxvq_u8(invalid_mask) != 0 {
            return false; // Invalid characters
        }

        // 4. Translate: indices = input + delta_rebase[hash_key]
        let indices = vaddq_u8(input_vec, vqtbl1q_u8(delta_rebase, hash_key));

        // Pack 5-bit values into bytes (16 chars -> 10 bytes)
        let decoded = pack_5bit_to_8bit_neon(indices);

        // Store 10 bytes
        let mut output_buf = [0u8; 16];
        vst1q_u8(output_buf.as_mut_ptr(), decoded);
        result.extend_from_slice(&output_buf[0..10]);
    }

    // Handle remainder with scalar fallback
    if simd_bytes < input_no_padding.len() {
        let remainder = &input_no_padding[simd_bytes..];
        if !decode_scalar_remainder(
            remainder,
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
            result,
        ) {
            return false;
        }
    }

    true
}

/// Get decode delta tables for hash-based validation (NEON)
///
/// Returns (delta_check, delta_rebase) lookup tables indexed by high nibble.
/// These tables enable single-shuffle validation and translation.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn get_decode_delta_tables_neon(
    variant: Base32Variant,
) -> (
    std::arch::aarch64::uint8x16_t,
    std::arch::aarch64::uint8x16_t,
) {
    use std::arch::aarch64::*;

    match variant {
        Base32Variant::Rfc4648 => {
            // RFC 4648 standard: A-Z (0x41-0x5A) -> 0-25, 2-7 (0x32-0x37) -> 26-31
            // Hash key is high nibble (input >> 4)
            //
            // High nibble ranges:
            // 0x3x: '2'-'7' (0x32-0x37)
            // 0x4x: 'A'-'O' (0x41-0x4F)
            // 0x5x: 'P'-'Z' (0x50-0x5A)

            let delta_check = vld1q_u8(
                [
                    0x7F,
                    0x7F,
                    0x7F,                // 0x0, 0x1, 0x2 - invalid
                    (0x1F - 0x37) as u8, // 0x3: '2'-'7' -> check <= 0x1F
                    (0x1F - 0x4F) as u8, // 0x4: 'A'-'O' -> check <= 0x1F
                    (0x1F - 0x5A) as u8, // 0x5: 'P'-'Z' -> check <= 0x1F
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F, // 0x6-0xF - invalid
                ]
                .as_ptr(),
            );

            let delta_rebase = vld1q_u8(
                [
                    0,
                    0,
                    0,                           // 0x0, 0x1, 0x2 - unused
                    (26i16 - b'2' as i16) as u8, // 0x3: '2' -> 26
                    (0i16 - b'A' as i16) as u8,  // 0x4: 'A' -> 0
                    (0i16 - b'A' as i16) as u8,  // 0x5: 'A' -> 0 (P-Z use same offset)
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
                ]
                .as_ptr(),
            );

            (delta_check, delta_rebase)
        }
        Base32Variant::Rfc4648Hex => {
            // RFC 4648 hex: 0-9 (0x30-0x39) -> 0-9, A-V (0x41-0x56) -> 10-31
            // High nibble ranges:
            // 0x3x: '0'-'9' (0x30-0x39)
            // 0x4x: 'A'-'O' (0x41-0x4F)
            // 0x5x: 'P'-'V' (0x50-0x56)

            let delta_check = vld1q_u8(
                [
                    0x7F,
                    0x7F,
                    0x7F,                // 0x0, 0x1, 0x2 - invalid
                    (0x1F - 0x39) as u8, // 0x3: '0'-'9' -> check <= 0x1F
                    (0x1F - 0x4F) as u8, // 0x4: 'A'-'O' -> check <= 0x1F
                    (0x1F - 0x56) as u8, // 0x5: 'P'-'V' -> check <= 0x1F
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F,
                    0x7F, // 0x6-0xF - invalid
                ]
                .as_ptr(),
            );

            let delta_rebase = vld1q_u8(
                [
                    0,
                    0,
                    0,                           // 0x0, 0x1, 0x2 - unused
                    (0i16 - b'0' as i16) as u8,  // 0x3: '0' -> 0
                    (10i16 - b'A' as i16) as u8, // 0x4: 'A' -> 10
                    (10i16 - b'A' as i16) as u8, // 0x5: 'A' -> 10 (P-V use same offset)
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
                ]
                .as_ptr(),
            );

            (delta_check, delta_rebase)
        }
    }
}

/// Pack 16 bytes of 5-bit indices into 10 bytes (NEON)
///
/// Uses direct bit manipulation since NEON lacks direct equivalents to
/// x86's _mm_maddubs_epi16 and _mm_madd_epi16 intrinsics.
/// 16 5-bit values -> 10 8-bit bytes
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn pack_5bit_to_8bit_neon(
    indices: std::arch::aarch64::uint8x16_t,
) -> std::arch::aarch64::uint8x16_t {
    use std::arch::aarch64::*;

    // Extract indices to buffer for bit manipulation
    let mut idx = [0u8; 16];
    vst1q_u8(idx.as_mut_ptr(), indices);

    let mut out = [0u8; 16];

    // Pack first group: 8 x 5-bit values -> 5 bytes
    // Bits: [4:0][4:0][4:0][4:0][4:0][4:0][4:0][4:0] -> 40 bits = 5 bytes
    out[0] = (idx[0] << 3) | (idx[1] >> 2);
    out[1] = (idx[1] << 6) | (idx[2] << 1) | (idx[3] >> 4);
    out[2] = (idx[3] << 4) | (idx[4] >> 1);
    out[3] = (idx[4] << 7) | (idx[5] << 2) | (idx[6] >> 3);
    out[4] = (idx[6] << 5) | idx[7];

    // Pack second group: 8 x 5-bit values -> 5 bytes
    out[5] = (idx[8] << 3) | (idx[9] >> 2);
    out[6] = (idx[9] << 6) | (idx[10] << 1) | (idx[11] >> 4);
    out[7] = (idx[11] << 4) | (idx[12] >> 1);
    out[8] = (idx[12] << 7) | (idx[13] << 2) | (idx[14] >> 3);
    out[9] = (idx[14] << 5) | idx[15];

    vld1q_u8(out.as_ptr())
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
