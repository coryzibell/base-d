//! NEON-accelerated base64 encoding and decoding
//!
//! ARM NEON implementation of base64, matching the x86_64 SSSE3/AVX2 algorithm.
//! Processes 12 input bytes -> 16 output characters per iteration.

use crate::core::dictionary::Dictionary;
use crate::simd::alphabets::AlphabetVariant;
use std::arch::aarch64::uint8x16_t;

/// NEON-accelerated base64 encoding
///
/// Processes 12 input bytes -> 16 output characters per iteration.
/// Falls back to scalar for remainder.
#[cfg(target_arch = "aarch64")]
pub fn encode(data: &[u8], dictionary: &Dictionary, variant: AlphabetVariant) -> Option<String> {
    let output_len = ((data.len() + 2) / 3) * 4;
    let mut result = String::with_capacity(output_len);

    unsafe {
        encode_neon_impl(data, dictionary, variant, &mut result);
    }

    Some(result)
}

/// NEON-accelerated base64 decoding
///
/// Processes 16 input characters -> 12 output bytes per iteration.
/// Falls back to scalar for remainder.
#[cfg(target_arch = "aarch64")]
pub fn decode(encoded: &str, variant: AlphabetVariant) -> Option<Vec<u8>> {
    let encoded_bytes = encoded.as_bytes();

    let input_no_padding = encoded.trim_end_matches('=');
    let output_len = (input_no_padding.len() / 4) * 3
        + match input_no_padding.len() % 4 {
            0 => 0,
            2 => 1,
            3 => 2,
            _ => return None,
        };

    let mut result = Vec::with_capacity(output_len);

    unsafe {
        if !decode_neon_impl(encoded_bytes, variant, &mut result) {
            return None;
        }
    }

    Some(result)
}

/// NEON base64 encoding implementation
///
/// Processes 12 input bytes -> 16 output characters per iteration.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn encode_neon_impl(
    data: &[u8],
    dictionary: &Dictionary,
    variant: AlphabetVariant,
    result: &mut String,
) {
    use std::arch::aarch64::*;

    const BLOCK_SIZE: usize = 12;

    if data.len() < 16 {
        encode_scalar_remainder(data, dictionary, result);
        return;
    }

    let safe_len = if data.len() >= 4 { data.len() - 4 } else { 0 };
    let num_blocks = safe_len / BLOCK_SIZE;
    let simd_bytes = num_blocks * BLOCK_SIZE;

    let mut offset = 0;
    for _ in 0..num_blocks {
        let input_vec = vld1q_u8(data.as_ptr().add(offset));
        let reshuffled = reshuffle_neon(input_vec);
        let encoded = translate_neon(reshuffled, variant);

        let mut output_buf = [0u8; 16];
        vst1q_u8(output_buf.as_mut_ptr(), encoded);

        for &byte in &output_buf {
            result.push(byte as char);
        }

        offset += BLOCK_SIZE;
    }

    if simd_bytes < data.len() {
        encode_scalar_remainder(&data[simd_bytes..], dictionary, result);
    }
}

/// Reshuffle bytes and extract 6-bit indices from 12 input bytes (NEON)
///
/// Equivalent to x86_64 SSSE3 reshuffle, using NEON intrinsics.
/// Uses vqtbl1q_u8 (table lookup) instead of pshufb.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn reshuffle_neon(input: uint8x16_t) -> uint8x16_t {
    use std::arch::aarch64::*;

    // Shuffle mask: Duplicate bytes to prepare for 6-bit extraction
    // Each group of 3 input bytes becomes 4 output bytes with duplicates
    let shuffle_indices = vld1q_u8(
        [
            0, 0, 1, 2, // bytes 0-2 -> positions 0-3
            3, 3, 4, 5, // bytes 3-5 -> positions 4-7
            6, 6, 7, 8, // bytes 6-8 -> positions 8-11
            9, 9, 10, 11, // bytes 9-11 -> positions 12-15
        ]
        .as_ptr(),
    );

    let shuffled = vqtbl1q_u8(input, shuffle_indices);

    // Extract 6-bit groups using shifts and masks
    // Pattern for 3 bytes ABC (24 bits) -> 4x 6-bit values:
    // [AAAAAA??] [??BBBBBB] [????CCCC] [CC??????]
    // After shuffle, we have bytes duplicated to allow extraction

    let shuffled_u32 = vreinterpretq_u32_u8(shuffled);

    // First extraction: positions 0 and 2 in each group
    // Mask 0x0FC0FC00: isolate specific bit positions
    let t0 = vandq_u32(shuffled_u32, vdupq_n_u32(0x0FC0FC00));

    // Simulate mulhi_epu16: multiply and extract high bits
    // For NEON, use shifts to achieve same effect
    let t0_u16 = vreinterpretq_u16_u32(t0);
    let mult_hi = vmulq_n_u16(t0_u16, 0x0040);
    let t1 = vreinterpretq_u32_u16(vshrq_n_u16(mult_hi, 10));

    // Second extraction: positions 1 and 3 in each group
    // Mask 0x003F03F0: isolate different bit positions
    let t2 = vandq_u32(shuffled_u32, vdupq_n_u32(0x003F03F0));
    let t2_u16 = vreinterpretq_u16_u32(t2);
    let mult_lo = vmulq_n_u16(t2_u16, 0x0010);
    let t3 = vreinterpretq_u32_u16(vshrq_n_u16(mult_lo, 6));

    // Combine the two results
    vreinterpretq_u8_u32(vorrq_u32(t1, t3))
}

/// Translate 6-bit indices to base64 ASCII characters (NEON)
///
/// Equivalent to x86_64 SSSE3 translate, using NEON vqtbl4q_u8 for 64-entry LUT.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn translate_neon(indices: uint8x16_t, variant: AlphabetVariant) -> uint8x16_t {
    use std::arch::aarch64::*;

    // Offset-based approach (same as x86_64)
    let lut = match variant {
        AlphabetVariant::Base64Standard => vld1q_u8(
            [
                65, 71, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 237, 240, 0, 0,
            ]
            .as_ptr(),
        ),
        AlphabetVariant::Base64Url => vld1q_u8(
            [
                65, 71, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 239, 32, 0, 0,
            ]
            .as_ptr(),
        ),
    };

    let mut lut_indices = vqsubq_u8(indices, vdupq_n_u8(51));
    let indices_signed = vreinterpretq_s8_u8(indices);
    let mask = vreinterpretq_u8_s8(vcgtq_s8(indices_signed, vdupq_n_s8(25)));
    lut_indices = vsubq_u8(lut_indices, mask);

    let offsets = vqtbl1q_u8(lut, lut_indices);
    vaddq_u8(indices, offsets)
}

/// NEON base64 decoding implementation
///
/// Processes 16 input characters -> 12 output bytes per iteration.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn decode_neon_impl(encoded: &[u8], variant: AlphabetVariant, result: &mut Vec<u8>) -> bool {
    use std::arch::aarch64::*;

    const INPUT_BLOCK_SIZE: usize = 16;
    const OUTPUT_BLOCK_SIZE: usize = 12;

    let input_no_padding = if let Some(last_non_pad) = encoded.iter().rposition(|&b| b != b'=') {
        &encoded[..=last_non_pad]
    } else {
        encoded
    };

    let (lut_lo, lut_hi, lut_roll) = get_decode_luts_neon(variant);

    let num_blocks = input_no_padding.len() / INPUT_BLOCK_SIZE;
    let simd_bytes = num_blocks * INPUT_BLOCK_SIZE;

    for round in 0..num_blocks {
        let offset = round * INPUT_BLOCK_SIZE;
        let input_vec = vld1q_u8(input_no_padding.as_ptr().add(offset));

        if !validate_neon(input_vec, lut_lo, lut_hi) {
            return false;
        }

        let indices = translate_decode_neon(input_vec, lut_hi, lut_roll);
        let decoded = reshuffle_decode_neon(indices);

        let mut output_buf = [0u8; 16];
        vst1q_u8(output_buf.as_mut_ptr(), decoded);
        result.extend_from_slice(&output_buf[0..OUTPUT_BLOCK_SIZE]);
    }

    if simd_bytes < input_no_padding.len() {
        let remainder = &input_no_padding[simd_bytes..];
        if !decode_scalar_remainder(
            remainder,
            &mut |c| match c {
                b'A'..=b'Z' => Some((c - b'A') as u8),
                b'a'..=b'z' => Some((c - b'a' + 26) as u8),
                b'0'..=b'9' => Some((c - b'0' + 52) as u8),
                b'+' if matches!(variant, AlphabetVariant::Base64Standard) => Some(62),
                b'/' if matches!(variant, AlphabetVariant::Base64Standard) => Some(63),
                b'-' if matches!(variant, AlphabetVariant::Base64Url) => Some(62),
                b'_' if matches!(variant, AlphabetVariant::Base64Url) => Some(63),
                _ => None,
            },
            result,
        ) {
            return false;
        }
    }

    true
}

/// Get decode lookup tables for NEON
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn get_decode_luts_neon(variant: AlphabetVariant) -> (uint8x16_t, uint8x16_t, uint8x16_t) {
    use std::arch::aarch64::*;

    let lut_lo = vld1q_u8(
        [
            0x15, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x13, 0x1A, 0x1B, 0x1B,
            0x1B, 0x1A,
        ]
        .as_ptr(),
    );

    let lut_hi = vld1q_u8(
        [
            0x10, 0x10, 0x01, 0x02, 0x04, 0x08, 0x04, 0x08, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10,
            0x10, 0x10,
        ]
        .as_ptr(),
    );

    let lut_roll = match variant {
        AlphabetVariant::Base64Standard => {
            vld1q_u8([0, 16, 19, 4, 191, 191, 185, 185, 0, 0, 0, 0, 0, 0, 0, 0].as_ptr())
        }
        AlphabetVariant::Base64Url => {
            vld1q_u8([0, 17, 224, 4, 191, 191, 185, 185, 0, 0, 0, 0, 0, 0, 0, 0].as_ptr())
        }
    };

    (lut_lo, lut_hi, lut_roll)
}

/// Validate input characters (NEON)
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn validate_neon(input: uint8x16_t, lut_lo: uint8x16_t, lut_hi: uint8x16_t) -> bool {
    use std::arch::aarch64::*;

    let lo_nibbles = vandq_u8(input, vdupq_n_u8(0x0F));
    let hi_nibbles = vandq_u8(vshrq_n_u8(input, 4), vdupq_n_u8(0x0F));

    let lo_lookup = vqtbl1q_u8(lut_lo, lo_nibbles);
    let hi_lookup = vqtbl1q_u8(lut_hi, hi_nibbles);

    let validation = vandq_u8(lo_lookup, hi_lookup);

    // Check if all bytes are 0 (no movemask in NEON, use vmaxvq)
    vmaxvq_u8(validation) == 0
}

/// Translate ASCII to 6-bit indices (NEON)
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn translate_decode_neon(
    input: uint8x16_t,
    _lut_hi: uint8x16_t,
    lut_roll: uint8x16_t,
) -> uint8x16_t {
    use std::arch::aarch64::*;

    let hi_nibbles = vandq_u8(vshrq_n_u8(input, 4), vdupq_n_u8(0x0F));
    let eq_2f = vceqq_u8(input, vdupq_n_u8(0x2F));
    let roll_index = vaddq_u8(eq_2f, hi_nibbles);
    let offsets = vqtbl1q_u8(lut_roll, roll_index);

    vaddq_u8(input, offsets)
}

/// Reshuffle 6-bit indices to packed 8-bit bytes (NEON)
///
/// Converts 16 bytes of 6-bit values to 12 bytes of 8-bit data.
/// Equivalent to x86_64 SSSE3 reshuffle_decode.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn reshuffle_decode_neon(indices: uint8x16_t) -> uint8x16_t {
    use std::arch::aarch64::*;

    // Simulate _mm_maddubs_epi16: multiply adjacent u8 pairs and add
    // Input: [a0 b0 a1 b1 a2 b2 ...] u8x16
    // Multiply pattern: [0x01 0x40 0x01 0x40 ...]
    // Result: [a0*1 + b0*64, a1*1 + b1*64, ...] as i16x8

    let pairs = vreinterpretq_u16_u8(indices);

    // Extract even bytes (a0, a1, a2, ...) and odd bytes (b0, b1, b2, ...)
    let even = vandq_u16(pairs, vdupq_n_u16(0xFF)); // Low byte of each pair
    let odd = vshrq_n_u16(pairs, 8); // High byte of each pair

    // Stage 1: merge_ab_and_bc = a + (b << 6)
    let merge_result = vaddq_u16(even, vshlq_n_u16(odd, 6));

    // Stage 2: Combine 16-bit pairs using multiply-add
    // _mm_madd_epi16: multiply adjacent i16 and add horizontally
    // Pattern: [0x1000 0x0001 0x1000 0x0001 ...]
    // Result: [p0*0x1000 + p1*0x0001, ...] as i32x4

    let merge_u32 = vreinterpretq_u32_u16(merge_result);

    // Extract low and high 16-bit values from each 32-bit pair
    let lo = vandq_u32(merge_u32, vdupq_n_u32(0xFFFF));
    let hi = vshrq_n_u32(merge_u32, 16);

    // Combine: lo << 12 | hi
    let final_32bit = vorrq_u32(vshlq_n_u32(lo, 12), hi);

    // Stage 3: Extract valid bytes (3 bytes per 32-bit group)
    let shuffle_mask = vld1q_u8(
        [
            2, 1, 0, // first group (reversed byte order)
            6, 5, 4, // second group
            10, 9, 8, // third group
            14, 13, 12, // fourth group
            255, 255, 255, 255,
        ]
        .as_ptr(),
    );

    let result_bytes = vreinterpretq_u8_u32(final_32bit);
    vqtbl1q_u8(result_bytes, shuffle_mask)
}

/// Encode remaining bytes using scalar algorithm
#[allow(dead_code)]
fn encode_scalar_remainder(data: &[u8], dictionary: &Dictionary, result: &mut String) {
    // Inline the scalar encoding to avoid cross-platform module dependency
    let base = dictionary.base();
    let bits_per_char = (base as f64).log2() as usize;

    if bits_per_char == 0 || bits_per_char > 8 {
        return;
    }

    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0;

    for &byte in data {
        bit_buffer = (bit_buffer << 8) | (byte as u32);
        bits_in_buffer += 8;

        while bits_in_buffer >= bits_per_char {
            bits_in_buffer -= bits_per_char;
            let index = ((bit_buffer >> bits_in_buffer) & ((1 << bits_per_char) - 1)) as usize;
            if let Some(ch) = dictionary.encode_digit(index) {
                result.push(ch);
            }
        }
    }

    if bits_in_buffer > 0 {
        let index = ((bit_buffer << (bits_per_char - bits_in_buffer)) & ((1 << bits_per_char) - 1))
            as usize;
        if let Some(ch) = dictionary.encode_digit(index) {
            result.push(ch);
        }
    }
}

/// Decode remaining bytes using scalar algorithm
#[allow(dead_code)]
fn decode_scalar_remainder(
    data: &[u8],
    char_to_index: &mut dyn FnMut(u8) -> Option<u8>,
    result: &mut Vec<u8>,
) -> bool {
    // Inline the scalar decoding to avoid cross-platform module dependency
    let bits_per_char = 6; // base64
    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0;

    for &byte in data {
        let Some(value) = char_to_index(byte) else {
            return false;
        };

        bit_buffer = (bit_buffer << bits_per_char) | (value as u32);
        bits_in_buffer += bits_per_char;

        while bits_in_buffer >= 8 {
            bits_in_buffer -= 8;
            let output_byte = ((bit_buffer >> bits_in_buffer) & 0xFF) as u8;
            result.push(output_byte);
        }
    }

    true
}
