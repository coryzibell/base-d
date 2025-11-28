//! NEON-accelerated base16/hex encoding and decoding
//!
//! Placeholder implementation for ARM NEON base16 operations.

use crate::core::dictionary::Dictionary;

/// Hex dictionary variants supported by specialized SIMD implementations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexVariant {
    /// Uppercase hex: 0-9A-F
    Uppercase,
    /// Lowercase hex: 0-9a-f
    Lowercase,
}

/// Identify which hex variant the dictionary represents
pub fn identify_hex_variant(dict: &Dictionary) -> Option<HexVariant> {
    if dict.base() != 16 {
        return None;
    }

    // Check uppercase: 0-9A-F
    let uppercase = "0123456789ABCDEF";
    if uppercase
        .chars()
        .enumerate()
        .all(|(i, c)| dict.encode_digit(i) == Some(c))
    {
        return Some(HexVariant::Uppercase);
    }

    // Check lowercase: 0-9a-f
    let lowercase = "0123456789abcdef";
    if lowercase
        .chars()
        .enumerate()
        .all(|(i, c)| dict.encode_digit(i) == Some(c))
    {
        return Some(HexVariant::Lowercase);
    }

    None
}

/// NEON-accelerated base16 encoding
///
/// Algorithm:
/// 1. Load 16 bytes
/// 2. Split each byte into high/low nibbles
/// 3. Translate nibbles (0-15) to ASCII hex characters using vqtbl1q_u8
/// 4. Interleave high/low nibbles using vzip1q_u8/vzip2q_u8
/// 5. Store 32 hex characters
#[cfg(target_arch = "aarch64")]
pub fn encode(data: &[u8], _dictionary: &Dictionary, variant: HexVariant) -> Option<String> {
    let output_len = data.len() * 2;
    let mut result = String::with_capacity(output_len);

    // SAFETY: NEON is mandatory on aarch64
    unsafe {
        encode_neon_impl(data, variant, &mut result);
    }

    Some(result)
}

/// NEON-accelerated base16 decoding
///
/// Algorithm:
/// 1. Load 32 hex chars
/// 2. Validate (0-9, A-F, a-f only)
/// 3. Translate ASCII → 0-15 values
/// 4. Pack pairs of nibbles into bytes (high << 4 | low)
/// 5. Store 16 bytes
#[cfg(target_arch = "aarch64")]
pub fn decode(encoded: &str, _variant: HexVariant) -> Option<Vec<u8>> {
    let encoded_bytes = encoded.as_bytes();

    // Hex must have even number of chars
    if encoded_bytes.len() % 2 != 0 {
        return None;
    }

    let output_len = encoded_bytes.len() / 2;
    let mut result = Vec::with_capacity(output_len);

    // SAFETY: NEON is mandatory on aarch64
    unsafe {
        if !decode_neon_impl(encoded_bytes, &mut result) {
            return None;
        }
    }

    Some(result)
}

/// NEON base16 encoding implementation
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn encode_neon_impl(data: &[u8], variant: HexVariant, result: &mut String) {
    use std::arch::aarch64::*;

    const BLOCK_SIZE: usize = 16;

    if data.len() < BLOCK_SIZE {
        encode_scalar_remainder(data, variant, result);
        return;
    }

    let num_blocks = data.len() / BLOCK_SIZE;
    let simd_bytes = num_blocks * BLOCK_SIZE;

    // Lookup table for hex digits (16 bytes)
    let lut = match variant {
        HexVariant::Uppercase => [
            b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'A', b'B', b'C', b'D',
            b'E', b'F',
        ],
        HexVariant::Lowercase => [
            b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'a', b'b', b'c', b'd',
            b'e', b'f',
        ],
    };
    let lut_vec = vld1q_u8(lut.as_ptr());

    let mask_0f = vdupq_n_u8(0x0F);

    let mut offset = 0;
    for _ in 0..num_blocks {
        // Load 16 bytes
        let input_vec = vld1q_u8(data.as_ptr().add(offset));

        // Extract high nibbles (shift right by 4)
        let hi_nibbles = vandq_u8(vshrq_n_u8(input_vec, 4), mask_0f);

        // Extract low nibbles
        let lo_nibbles = vandq_u8(input_vec, mask_0f);

        // Translate nibbles to ASCII using table lookup
        let hi_ascii = vqtbl1q_u8(lut_vec, hi_nibbles);
        let lo_ascii = vqtbl1q_u8(lut_vec, lo_nibbles);

        // Interleave high and low bytes: hi[0], lo[0], hi[1], lo[1], ...
        // NEON advantage: vzip1q_u8/vzip2q_u8 are cleaner than x86 unpack
        let result_lo = vzip1q_u8(hi_ascii, lo_ascii);
        let result_hi = vzip2q_u8(hi_ascii, lo_ascii);

        // Store 32 output characters
        let mut output_buf = [0u8; 32];
        vst1q_u8(output_buf.as_mut_ptr(), result_lo);
        vst1q_u8(output_buf.as_mut_ptr().add(16), result_hi);

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

/// NEON base16 decoding implementation
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn decode_neon_impl(encoded: &[u8], result: &mut Vec<u8>) -> bool {
    use std::arch::aarch64::*;

    const INPUT_BLOCK_SIZE: usize = 32;
    const OUTPUT_BLOCK_SIZE: usize = 16;

    if encoded.len() < INPUT_BLOCK_SIZE {
        return decode_scalar_remainder(encoded, result);
    }

    let num_blocks = encoded.len() / INPUT_BLOCK_SIZE;
    let simd_bytes = num_blocks * INPUT_BLOCK_SIZE;

    // Process full blocks
    for round in 0..num_blocks {
        let mut offset = round * INPUT_BLOCK_SIZE;

        // Load 32 bytes (16 pairs of hex chars)
        let input_lo = vld1q_u8(encoded.as_ptr().add(offset));
        let input_hi = vld1q_u8(encoded.as_ptr().add(offset + 16));

        // Deinterleave: separate high and low nibble chars
        // Input: [h0,l0, h1,l1, ...] → [h0,h1,...], [l0,l1,...]
        let deinterleaved = vuzpq_u8(input_lo, input_hi);
        let hi_chars = deinterleaved.0;
        let lo_chars = deinterleaved.1;

        // Decode both nibble streams
        let hi_vals = decode_nibble_chars_neon(hi_chars);
        let lo_vals = decode_nibble_chars_neon(lo_chars);

        // Check for invalid characters (255 in decoded values)
        // Use vmaxvq_u8 to find maximum value in vector
        if vmaxvq_u8(vceqq_u8(hi_vals, vdupq_n_u8(255))) != 0 {
            return false;
        }
        if vmaxvq_u8(vceqq_u8(lo_vals, vdupq_n_u8(255))) != 0 {
            return false;
        }

        // Pack nibbles into bytes: (high << 4) | low
        let packed = vorrq_u8(vshlq_n_u8(hi_vals, 4), lo_vals);

        // Store 16 bytes
        let mut output_buf = [0u8; OUTPUT_BLOCK_SIZE];
        vst1q_u8(output_buf.as_mut_ptr(), packed);
        result.extend_from_slice(&output_buf);

        offset += INPUT_BLOCK_SIZE;
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
/// Returns 255 for invalid characters
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn decode_nibble_chars_neon(
    chars: std::arch::aarch64::uint8x16_t,
) -> std::arch::aarch64::uint8x16_t {
    use std::arch::aarch64::*;

    // Strategy: Use character ranges to select appropriate offset
    // '0'-'9': 0x30-0x39 → subtract 0x30 → 0-9
    // 'A'-'F': 0x41-0x46 → subtract 0x37 → 10-15
    // 'a'-'f': 0x61-0x66 → subtract 0x57 → 10-15

    let zero_30 = vdupq_n_u8(0x30);
    let nine_39 = vdupq_n_u8(0x39);
    let a_41 = vdupq_n_u8(0x41);
    let f_46 = vdupq_n_u8(0x46);
    let a_61 = vdupq_n_u8(0x61);
    let f_66 = vdupq_n_u8(0x66);

    // Check if char is a digit ('0'-'9')
    let is_digit = vandq_u8(vcgeq_u8(chars, zero_30), vcleq_u8(chars, nine_39));

    // Check if char is uppercase hex ('A'-'F')
    let is_upper = vandq_u8(vcgeq_u8(chars, a_41), vcleq_u8(chars, f_46));

    // Check if char is lowercase hex ('a'-'f')
    let is_lower = vandq_u8(vcgeq_u8(chars, a_61), vcleq_u8(chars, f_66));

    // Decode using appropriate offset
    let digit_vals = vandq_u8(is_digit, vsubq_u8(chars, vdupq_n_u8(0x30)));
    let upper_vals = vandq_u8(is_upper, vsubq_u8(chars, vdupq_n_u8(0x37)));
    let lower_vals = vandq_u8(is_lower, vsubq_u8(chars, vdupq_n_u8(0x57)));

    // Combine results (only one should be non-zero per byte)
    let valid_vals = vorrq_u8(vorrq_u8(digit_vals, upper_vals), lower_vals);

    // Set invalid chars to 255
    let is_valid = vorrq_u8(vorrq_u8(is_digit, is_upper), is_lower);
    vorrq_u8(
        vandq_u8(is_valid, valid_vals),
        vbicq_u8(vdupq_n_u8(255), is_valid),
    )
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
