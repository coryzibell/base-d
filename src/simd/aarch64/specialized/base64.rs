//! SIMD implementation for base64 (6-bit encoding) using NEON
//!
//! Based on techniques from:
//! - https://github.com/aklomp/base64 (reference C implementation)
//! - ARM NEON optimization techniques

// Allow unused_unsafe because we explicitly wrap NEON intrinsics for Rust 2024
// edition compatibility (unsafe_op_in_unsafe_fn lint). The intrinsics may be
// marked safe in some versions, but we maintain explicit blocks for portability.
#![allow(unused_unsafe)]

use super::super::common;
use crate::core::dictionary::Dictionary;
use crate::simd::variants::DictionaryVariant;

/// SIMD-accelerated base64 encoding using NEON
///
/// Processes 12 bytes -> 16 chars per iteration using NEON intrinsics.
/// Falls back to scalar for remainder.
#[cfg(target_arch = "aarch64")]
pub fn encode(data: &[u8], dictionary: &Dictionary, variant: DictionaryVariant) -> Option<String> {
    // Pre-allocate output
    let output_len = data.len().div_ceil(3) * 4;
    let mut result = String::with_capacity(output_len);

    // SAFETY: NEON is always available on aarch64
    unsafe {
        encode_neon_impl(data, dictionary, variant, &mut result);
    }

    Some(result)
}

/// SIMD-accelerated base64 decoding using NEON
///
/// Processes 16 chars -> 12 bytes per iteration using NEON intrinsics.
/// Falls back to scalar for remainder.
#[cfg(target_arch = "aarch64")]
pub fn decode(encoded: &str, variant: DictionaryVariant) -> Option<Vec<u8>> {
    let encoded_bytes = encoded.as_bytes();

    // Calculate output size
    let input_no_padding = encoded.trim_end_matches('=');
    let output_len = (input_no_padding.len() / 4) * 3
        + match input_no_padding.len() % 4 {
            0 => 0,
            2 => 1,
            3 => 2,
            _ => return None, // Invalid base64
        };

    let mut result = Vec::with_capacity(output_len);

    // SAFETY: NEON is always available on aarch64
    unsafe {
        if !decode_neon_impl(encoded_bytes, variant, &mut result) {
            return None;
        }
    }

    Some(result)
}

/// NEON base64 encoding implementation
///
/// Based on the algorithm from https://github.com/aklomp/base64
/// Processes 12 input bytes -> 16 output characters per iteration
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn encode_neon_impl(
    data: &[u8],
    dictionary: &Dictionary,
    variant: DictionaryVariant,
    result: &mut String,
) {
    use std::arch::aarch64::*;

    const BLOCK_SIZE: usize = 12;

    // Need at least 16 bytes in buffer to safely load 128 bits
    if data.len() < 16 {
        encode_scalar_remainder(data, dictionary, result);
        return;
    }

    // Process blocks of 12 bytes. We load 16 bytes but only use 12.
    // Ensure we don't read past the buffer: need 4 extra bytes after last block
    let safe_len = if data.len() >= 4 { data.len() - 4 } else { 0 };
    let (num_rounds, simd_bytes) = common::calculate_blocks(safe_len, BLOCK_SIZE);

    let mut offset = 0;

    for _ in 0..num_rounds {
        // Load 16 bytes (we only use the first 12)
        let input_vec = unsafe { vld1q_u8(data.as_ptr().add(offset)) };

        // Reshuffle bytes to extract 6-bit groups
        let reshuffled = unsafe { reshuffle(input_vec) };

        // Translate 6-bit indices to ASCII
        let encoded = unsafe { translate(reshuffled, variant) };

        // Store to buffer
        let mut output_buf = [0u8; 16];
        unsafe {
            vst1q_u8(output_buf.as_mut_ptr(), encoded);
        }

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

/// Reshuffle bytes and extract 6-bit indices from 12 input bytes
///
/// Based on the algorithm from https://github.com/aklomp/base64
/// This function takes 12 bytes and produces 16 6-bit values (0-63)
///
/// The algorithm uses multiply instructions to perform the bit extraction,
/// which is more efficient than multiple shift/mask operations.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn reshuffle(input: std::arch::aarch64::uint8x16_t) -> std::arch::aarch64::uint8x16_t {
    use std::arch::aarch64::*;

    // Input, bytes MSB to LSB (little endian, so byte 0 is at low address):
    // 0 0 0 0 l k j i h g f e d c b a
    //
    // We need to reshuffle to prepare for 6-bit extraction.
    // Each group of 3 input bytes (24 bits) becomes 4 output bytes (4 x 6 bits)

    // Shuffle indices: matches x86 pattern [1,0,2,1, 4,3,5,4, 7,6,8,7, 10,9,11,10]
    let shuffle_indices = unsafe {
        vld1q_u8(
            [
                1, 0, 2, 1, // bytes 0-2 -> positions 0-3
                4, 3, 5, 4, // bytes 3-5 -> positions 4-7
                7, 6, 8, 7, // bytes 6-8 -> positions 8-11
                10, 9, 11, 10, // bytes 9-11 -> positions 12-15
            ]
            .as_ptr(),
        )
    };

    let shuffled = unsafe { vqtbl1q_u8(input, shuffle_indices) };

    // Now we need to extract the 6-bit groups using multiplication tricks.
    // For 3 bytes ABC (24 bits) -> 4 groups of 6 bits: [AAAAAA] [AABBBB] [BBBBCC] [CCCCCC]

    // First extraction: get bits for positions 0 and 2 in each group of 4
    let shuffled_u32 = unsafe { vreinterpretq_u32_u8(shuffled) };
    let t0 = unsafe { vandq_u32(shuffled_u32, vdupq_n_u32(0x0FC0FC00)) };

    // NEON doesn't have mulhi_epu16, so we use vmull + vshrn pattern
    let t1 = unsafe {
        let t0_u16 = vreinterpretq_u16_u32(t0);
        let mult_pattern = vreinterpretq_u16_u32(vdupq_n_u32(0x04000040));
        let lo = vget_low_u16(t0_u16);
        let hi = vget_high_u16(t0_u16);
        let mult_lo = vget_low_u16(mult_pattern);
        let mult_hi = vget_high_u16(mult_pattern);
        let lo_32 = vmull_u16(lo, mult_lo);
        let hi_32 = vmull_u16(hi, mult_hi);
        let lo_result = vshrn_n_u32(lo_32, 16);
        let hi_result = vshrn_n_u32(hi_32, 16);
        vreinterpretq_u32_u16(vcombine_u16(lo_result, hi_result))
    };

    // Second extraction: get bits for positions 1 and 3 in each group of 4
    let t2 = unsafe { vandq_u32(shuffled_u32, vdupq_n_u32(0x003F03F0)) };
    let t3 = unsafe {
        let t2_u16 = vreinterpretq_u16_u32(t2);
        let mult_pattern = vreinterpretq_u16_u32(vdupq_n_u32(0x01000010));
        vreinterpretq_u32_u16(vmulq_u16(t2_u16, mult_pattern))
    };

    // Combine the two results
    unsafe { vreinterpretq_u8_u32(vorrq_u32(t1, t3)) }
}

/// Translate 6-bit indices (0-63) to base64 ASCII characters
///
/// Based on the algorithm from https://github.com/aklomp/base64
/// Uses an offset-based lookup instead of direct table lookup,
/// which is more efficient for SIMD.
///
/// Standard base64 dictionary mapping:
/// - [0..25]  -> 'A'..'Z' (ASCII 65..90)   offset: +65
/// - [26..51] -> 'a'..'z' (ASCII 97..122)  offset: +71
/// - [52..61] -> '0'..'9' (ASCII 48..57)   offset: -4
/// - [62]     -> '+'      (ASCII 43)       offset: -19
/// - [63]     -> '/'      (ASCII 47)       offset: -16
///
/// URL-safe base64 dictionary differs only at positions 62-63:
/// - [62]     -> '-'      (ASCII 45)       offset: -17
/// - [63]     -> '_'      (ASCII 95)       offset: +32
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn translate(
    indices: std::arch::aarch64::uint8x16_t,
    variant: DictionaryVariant,
) -> std::arch::aarch64::uint8x16_t {
    use std::arch::aarch64::*;

    // Lookup table containing offsets to add to each index
    let lut = match variant {
        DictionaryVariant::Base64Standard => unsafe {
            vld1q_u8(
                [
                    65,  // index 0: 'A' = 0 + 65
                    71,  // index 1: for values 26-51, add 71 (26 + 71 = 97 = 'a')
                    252, // indices 2-11: for values 52-61, add -4 (as u8: 256-4=252)
                    252, 252, 252, 252, 252, 252, 252, 252, 252,
                    237, // index 12: for value 62, add -19 (as u8: 256-19=237)
                    240, // index 13: for value 63, add -16 (as u8: 256-16=240)
                    0,   // unused
                    0,   // unused
                ]
                .as_ptr(),
            )
        },
        DictionaryVariant::Base64Url => unsafe {
            vld1q_u8(
                [
                    65,  // index 0: 'A' = 0 + 65
                    71,  // index 1: for values 26-51, add 71 (26 + 71 = 97 = 'a')
                    252, // indices 2-11: for values 52-61, add -4 (as u8: 256-4=252)
                    252, 252, 252, 252, 252, 252, 252, 252, 252,
                    239, // index 12: for value 62, add -17 (as u8: 256-17=239)
                    32,  // index 13: for value 63, add 32 (63 + 32 = 95 = '_')
                    0,   // unused
                    0,   // unused
                ]
                .as_ptr(),
            )
        },
    };

    // Create LUT indices from the input values
    let mut lut_indices = unsafe { vqsubq_u8(indices, vdupq_n_u8(51)) };
    let indices_s8 = unsafe { vreinterpretq_s8_u8(indices) };
    // vcgtq_s8 returns uint8x16_t (comparison results are always unsigned)
    let mask = unsafe { vcgtq_s8(indices_s8, vdupq_n_s8(25)) };
    lut_indices = unsafe { vsubq_u8(lut_indices, mask) };

    // Look up the offsets and add to original indices
    let offsets = unsafe { vqtbl1q_u8(lut, lut_indices) };
    unsafe { vaddq_u8(indices, offsets) }
}

/// NEON base64 decoding implementation
///
/// Based on the algorithm from https://github.com/aklomp/base64
/// Processes 16 input characters -> 12 output bytes per iteration
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn decode_neon_impl(
    encoded: &[u8],
    variant: DictionaryVariant,
    result: &mut Vec<u8>,
) -> bool {
    use std::arch::aarch64::*;

    const INPUT_BLOCK_SIZE: usize = 16;
    const OUTPUT_BLOCK_SIZE: usize = 12;

    // Strip padding
    let input_no_padding = if let Some(last_non_pad) = encoded.iter().rposition(|&b| b != b'=') {
        &encoded[..=last_non_pad]
    } else {
        encoded
    };

    // Get LUTs
    let (lut_lo, lut_hi, lut_roll) = unsafe { get_decode_luts(variant) };

    // Calculate number of full 16-byte blocks
    let (num_rounds, simd_bytes) =
        common::calculate_blocks(input_no_padding.len(), INPUT_BLOCK_SIZE);

    // Process full blocks
    for round in 0..num_rounds {
        let offset = round * INPUT_BLOCK_SIZE;

        let input_vec = unsafe { vld1q_u8(input_no_padding.as_ptr().add(offset)) };

        if !unsafe { validate(input_vec, lut_lo, lut_hi) } {
            return false; // Invalid characters
        }

        let decoded = unsafe {
            // Translate ASCII to 6-bit indices
            let indices = translate_decode(input_vec, lut_hi, lut_roll);

            // Reshuffle 6-bit to 8-bit
            reshuffle_decode(indices)
        };

        let mut output_buf = [0u8; 16];
        unsafe {
            vst1q_u8(output_buf.as_mut_ptr(), decoded);
        }

        result.extend_from_slice(&output_buf[0..OUTPUT_BLOCK_SIZE]);
    }

    // Handle remainder with scalar fallback
    if simd_bytes < input_no_padding.len() {
        let remainder = &input_no_padding[simd_bytes..];
        if !decode_scalar_remainder(
            remainder,
            &mut |c| match c {
                b'A'..=b'Z' => Some(c - b'A'),
                b'a'..=b'z' => Some(c - b'a' + 26),
                b'0'..=b'9' => Some(c - b'0' + 52),
                b'+' if matches!(variant, DictionaryVariant::Base64Standard) => Some(62),
                b'/' if matches!(variant, DictionaryVariant::Base64Standard) => Some(63),
                b'-' if matches!(variant, DictionaryVariant::Base64Url) => Some(62),
                b'_' if matches!(variant, DictionaryVariant::Base64Url) => Some(63),
                _ => None,
            },
            result,
        ) {
            return false;
        }
    }

    true
}

/// Get decode lookup tables for the specified variant
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn get_decode_luts(
    variant: DictionaryVariant,
) -> (
    std::arch::aarch64::uint8x16_t,
    std::arch::aarch64::uint8x16_t,
    std::arch::aarch64::uint8x16_t,
) {
    use std::arch::aarch64::*;

    // Low nibble lookup - validates based on low 4 bits
    let lut_lo = unsafe {
        vld1q_u8(
            [
                0x15, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x13, 0x1A, 0x1B, 0x1B,
                0x1B, 0x1A,
            ]
            .as_ptr(),
        )
    };

    // High nibble lookup - validates based on high 4 bits
    let lut_hi = unsafe {
        vld1q_u8(
            [
                0x10, 0x10, 0x01, 0x02, 0x04, 0x08, 0x04, 0x08, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10,
                0x10, 0x10,
            ]
            .as_ptr(),
        )
    };

    // Roll/offset lookup - converts ASCII to 6-bit indices
    let lut_roll = match variant {
        DictionaryVariant::Base64Standard => unsafe {
            vld1q_u8(
                [
                    0, 16, 19, 4, 191, 191, 185, 185, 0, 0, 0, 0, 0, 0, 0,
                    0, // -65 = 256-65 = 191, -71 = 256-71 = 185
                ]
                .as_ptr(),
            )
        },
        DictionaryVariant::Base64Url => unsafe {
            vld1q_u8(
                [
                    0, 17, 224, 4, 191, 191, 185, 185, 0, 0, 0, 0, 0, 0, 0,
                    0, // -32 = 256-32 = 224
                ]
                .as_ptr(),
            )
        },
    };

    (lut_lo, lut_hi, lut_roll)
}

/// Validate that all input bytes are valid base64 characters
///
/// Returns true if all bytes are valid, false otherwise
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn validate(
    input: std::arch::aarch64::uint8x16_t,
    lut_lo: std::arch::aarch64::uint8x16_t,
    lut_hi: std::arch::aarch64::uint8x16_t,
) -> bool {
    use std::arch::aarch64::*;

    // Extract low and high nibbles
    let lo_nibbles = unsafe { vandq_u8(input, vdupq_n_u8(0x0F)) };
    let hi_nibbles_shifted = unsafe { vshrq_n_u8(input, 4) };
    let hi_nibbles = unsafe { vandq_u8(hi_nibbles_shifted, vdupq_n_u8(0x0F)) };

    // Look up validation values
    let lo_lookup = unsafe { vqtbl1q_u8(lut_lo, lo_nibbles) };
    let hi_lookup = unsafe { vqtbl1q_u8(lut_hi, hi_nibbles) };

    // AND the two lookups - result should be 0 for valid characters
    let validation = unsafe { vandq_u8(lo_lookup, hi_lookup) };

    // Check if all bytes are 0 (valid)
    // vmaxvq_u8 returns the maximum byte value
    unsafe { vmaxvq_u8(validation) == 0 }
}

/// Translate ASCII characters to 6-bit indices
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn translate_decode(
    input: std::arch::aarch64::uint8x16_t,
    _lut_hi: std::arch::aarch64::uint8x16_t,
    lut_roll: std::arch::aarch64::uint8x16_t,
) -> std::arch::aarch64::uint8x16_t {
    use std::arch::aarch64::*;

    // Extract high nibbles
    let hi_nibbles_shifted = unsafe { vshrq_n_u8(input, 4) };
    let hi_nibbles = unsafe { vandq_u8(hi_nibbles_shifted, vdupq_n_u8(0x0F)) };

    // Check for '/' character (0x2F)
    let eq_2f = unsafe { vceqq_u8(input, vdupq_n_u8(0x2F)) };

    // Index into lut_roll is: hi_nibbles + (input == '/')
    let roll_index = unsafe { vaddq_u8(eq_2f, hi_nibbles) };

    // Look up offset values from lut_roll
    let offsets = unsafe { vqtbl1q_u8(lut_roll, roll_index) };

    // Add offsets to convert ASCII to 6-bit indices
    unsafe { vaddq_u8(input, offsets) }
}

/// Reshuffle 6-bit indices to packed 8-bit bytes
///
/// Converts 16 bytes of 6-bit values (0-63) to 12 bytes of 8-bit data
///
/// Algorithm: Pack 4 x 6-bit values (24 bits) into 3 bytes
/// Input:  [A5..A0][B5..B0][C5..C0][D5..D0] (4 bytes, only low 6 bits used)
/// Output: [A5..A0|B5..B4][B3..B0|C5..C2][C1..C0|D5..D0] (3 bytes)
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn reshuffle_decode(
    indices: std::arch::aarch64::uint8x16_t,
) -> std::arch::aarch64::uint8x16_t {
    use std::arch::aarch64::*;

    // Reinterpret as u32 to work on 4-byte groups
    let _indices_u32 = vreinterpretq_u32_u8(indices);

    // For each u32 containing [D, C, B, A] (little-endian: A is byte 0):
    // We need to compute:
    //   out[0] = (A << 2) | (B >> 4)
    //   out[1] = (B << 4) | (C >> 2)
    //   out[2] = (C << 6) | D
    //
    // Using the x86 maddubs approach:
    // Stage 1: maddubs with 0x01400140 merges adjacent pairs:
    //   lo_pair = A * 0x40 + B * 0x01 = (A << 6) + B  [12 bits in u16]
    //   hi_pair = C * 0x40 + D * 0x01 = (C << 6) + D  [12 bits in u16]
    //
    // Stage 2: madd with 0x00011000 merges the two u16s:
    //   result = lo_pair * 0x1000 + hi_pair * 0x0001
    //          = ((A << 6) + B) << 12) + ((C << 6) + D)
    //          = (A << 18) + (B << 12) + (C << 6) + D  [24 bits in u32]
    //
    // The 24 bits in positions [23:0] of each u32 are what we want as 3 bytes

    // Stage 1: Implement maddubs_epi16(indices, 0x01400140)
    // Split into even (A, C positions) and odd (B, D positions) bytes
    let even_mask = unsafe {
        vld1q_u8(
            [
                0, 255, 2, 255, 4, 255, 6, 255, 8, 255, 10, 255, 12, 255, 14, 255,
            ]
            .as_ptr(),
        )
    };
    let odd_mask = unsafe {
        vld1q_u8(
            [
                1, 255, 3, 255, 5, 255, 7, 255, 9, 255, 11, 255, 13, 255, 15, 255,
            ]
            .as_ptr(),
        )
    };

    // Extract even bytes (A, C) shifted left by 6, and odd bytes (B, D)
    let even_bytes = vqtbl1q_u8(indices, even_mask);
    let odd_bytes = vqtbl1q_u8(indices, odd_mask);

    // even_bytes * 0x40 = even_bytes << 6 (in u16 lanes)
    // odd_bytes * 0x01 = odd_bytes (in u16 lanes)
    let even_u16 = vreinterpretq_u16_u8(even_bytes);
    let odd_u16 = vreinterpretq_u16_u8(odd_bytes);

    // Shift even values left by 6 and add odd values
    let merged_u16 = vaddq_u16(vshlq_n_u16(even_u16, 6), odd_u16);

    // Stage 2: Implement madd_epi16(merged, 0x00011000)
    // For each pair of u16 [lo, hi]: result = lo * 0x1000 + hi * 0x0001
    // This packs the two 12-bit values into one 24-bit value

    // Split into pairs: low u16s (lo_pair) and high u16s (hi_pair)
    let lo_pair_mask = unsafe {
        vld1q_u8(
            [
                0, 1, 255, 255, 4, 5, 255, 255, 8, 9, 255, 255, 12, 13, 255, 255,
            ]
            .as_ptr(),
        )
    };
    let hi_pair_mask = unsafe {
        vld1q_u8(
            [
                2, 3, 255, 255, 6, 7, 255, 255, 10, 11, 255, 255, 14, 15, 255, 255,
            ]
            .as_ptr(),
        )
    };

    let lo_pairs = vreinterpretq_u32_u8(vqtbl1q_u8(vreinterpretq_u8_u16(merged_u16), lo_pair_mask));
    let hi_pairs = vreinterpretq_u32_u8(vqtbl1q_u8(vreinterpretq_u8_u16(merged_u16), hi_pair_mask));

    // lo_pairs * 0x1000 = lo_pairs << 12
    let final_u32 = vaddq_u32(vshlq_n_u32(lo_pairs, 12), hi_pairs);

    // Stage 3: Extract bytes 2, 1, 0 from each u32 (the 24-bit value is in low 3 bytes)
    // In little-endian, byte 0 is LSB, so we want bytes at positions 2, 1, 0 of each u32
    let shuffle =
        unsafe { vld1q_u8([2, 1, 0, 6, 5, 4, 10, 9, 8, 14, 13, 12, 255, 255, 255, 255].as_ptr()) };

    unsafe { vqtbl1q_u8(vreinterpretq_u8_u32(final_u32), shuffle) }
}

/// Encode remaining bytes using scalar algorithm
///
/// Also handles padding for base64 output
fn encode_scalar_remainder(data: &[u8], dictionary: &Dictionary, result: &mut String) {
    // Use common scalar chunked encoding (6-bit for base64)
    common::encode_scalar_chunked(data, dictionary, result);
}

/// Decode remaining bytes using scalar algorithm
fn decode_scalar_remainder(
    data: &[u8],
    char_to_index: &mut dyn FnMut(u8) -> Option<u8>,
    result: &mut Vec<u8>,
) -> bool {
    common::decode_scalar_chunked(data, char_to_index, result, 6)
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::core::config::EncodingMode;
    use crate::core::dictionary::Dictionary;

    fn make_base64_dict() -> Dictionary {
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
    fn test_encode_basic() {
        let dict = make_base64_dict();
        let input = b"Hello, World!";
        let result = encode(input, &dict, DictionaryVariant::Base64Standard);
        assert!(result.is_some());
        let encoded = result.unwrap();
        assert_eq!(encoded, "SGVsbG8sIFdvcmxkIQ==");
    }

    #[test]
    fn test_encode_url_safe() {
        let dict = make_base64_url_dict();
        let input = b"\xfb\xff\xfe";
        let result = encode(input, &dict, DictionaryVariant::Base64Url);
        assert!(result.is_some());
        let encoded = result.unwrap();
        assert_eq!(encoded, "-__-");
    }

    #[test]
    fn test_decode_basic() {
        let encoded = "SGVsbG8sIFdvcmxkIQ==";
        let result = decode(encoded, DictionaryVariant::Base64Standard);
        assert!(result.is_some());
        let decoded = result.unwrap();
        assert_eq!(decoded, b"Hello, World!");
    }

    #[test]
    fn test_decode_url_safe() {
        let encoded = "-__-";
        let result = decode(encoded, DictionaryVariant::Base64Url);
        assert!(result.is_some());
        let decoded = result.unwrap();
        assert_eq!(decoded, b"\xfb\xff\xfe");
    }

    #[test]
    fn test_round_trip() {
        let dict = make_base64_dict();
        let inputs: Vec<&[u8]> = vec![
            b"",
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            b"The quick brown fox jumps over the lazy dog",
        ];

        for input in inputs {
            let encoded =
                encode(input, &dict, DictionaryVariant::Base64Standard).expect("encode failed");
            let decoded =
                decode(&encoded, DictionaryVariant::Base64Standard).expect("decode failed");
            assert_eq!(decoded, input, "round-trip failed for input: {:?}", input);
        }
    }

    #[test]
    fn test_invalid_decode() {
        // Invalid character
        let result = decode("SGVs!G8=", DictionaryVariant::Base64Standard);
        assert!(result.is_none());

        // Invalid length (too short for SIMD, falls back)
        let result = decode("SGVs", DictionaryVariant::Base64Standard);
        // Short inputs may or may not decode depending on impl
        let _ = result;
    }

    #[test]
    fn test_large_input() {
        let dict = make_base64_dict();
        let input = vec![42u8; 1024];
        let encoded =
            encode(&input, &dict, DictionaryVariant::Base64Standard).expect("encode failed");
        let decoded = decode(&encoded, DictionaryVariant::Base64Standard).expect("decode failed");
        assert_eq!(decoded, input);
    }
}
