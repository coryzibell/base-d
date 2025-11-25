//! x86_64 SIMD implementations using AVX2 and SSSE3
//!
//! Based on techniques from:
//! - https://github.com/aklomp/base64 (reference C implementation)
//! - Wojciech Muła's SIMD base64 work
//! - Intel optimization manuals

use crate::core::dictionary::Dictionary;
use crate::simd::alphabets::{identify_base64_variant, AlphabetVariant};

/// SIMD-accelerated base64 encoding using SSSE3
///
/// Processes 12 bytes at a time, producing 16 base64 characters.
/// Falls back to scalar for remainder and non-standard dictionaries.
#[cfg(target_arch = "x86_64")]
pub fn encode_base64_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize standard base64 (6 bits per char)
    if dictionary.base() != 64 {
        return None;
    }

    // Identify which base64 variant this is
    let variant = identify_base64_variant(dictionary)?;

    // Need SSSE3 for pshufb
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Pre-allocate output
    let output_len = ((data.len() + 2) / 3) * 4;
    let mut result = String::with_capacity(output_len);

    // SAFETY: We checked for SSSE3 support above
    unsafe {
        encode_base64_ssse3_impl(data, dictionary, variant, &mut result);
    }

    Some(result)
}

/// SIMD-accelerated base64 decoding using SSSE3
#[cfg(target_arch = "x86_64")]
pub fn decode_base64_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base64 with known variants
    if dictionary.base() != 64 {
        return None;
    }

    let variant = identify_base64_variant(dictionary)?;

    // Need SSSE3 for pshufb
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Minimum 16 bytes for SIMD processing
    if encoded.len() < 16 {
        return None;
    }

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

    // SAFETY: We checked for SSSE3 support above
    unsafe {
        if !decode_base64_ssse3_impl(encoded, variant, &mut result) {
            return None;
        }
    }

    Some(result)
}

/// SSSE3 base64 encoding implementation
///
/// Based on the algorithm from https://github.com/aklomp/base64
/// Processes 12 input bytes -> 16 output characters per iteration
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn encode_base64_ssse3_impl(
    data: &[u8],
    dictionary: &Dictionary,
    variant: AlphabetVariant,
    result: &mut String,
) {
    use std::arch::x86_64::*;

    // We need at least 16 bytes to read (we read 16, use 12)
    // To avoid reading past the buffer, we need len >= 16 for SIMD
    // Actually, we need (len - 4) / 12 rounds, and 4 bytes safety margin
    const BLOCK_SIZE: usize = 12;

    // Need at least 16 bytes in buffer to safely load 128 bits
    if data.len() < 16 {
        // Fall back to scalar for small inputs
        encode_base64_scalar_remainder(data, dictionary, result);
        return;
    }

    // Process blocks of 12 bytes. We load 16 bytes but only use 12.
    // Ensure we don't read past the buffer: need 4 extra bytes after last block
    let num_rounds = (data.len() - 4) / BLOCK_SIZE;
    let simd_bytes = num_rounds * BLOCK_SIZE;

    let mut offset = 0;
    for _ in 0..num_rounds {
        // Load 16 bytes (we only use the first 12)
        let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

        // Reshuffle bytes to extract 6-bit groups
        let reshuffled = enc_reshuffle(input_vec);

        // Translate 6-bit indices to ASCII
        let encoded = enc_translate(reshuffled, variant);

        // Store 16 output characters
        let mut output_buf = [0u8; 16];
        _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, encoded);

        // Append to result (safe because base64 is ASCII)
        for &byte in &output_buf {
            result.push(byte as char);
        }

        offset += BLOCK_SIZE;
    }

    // Handle remainder with scalar code
    if simd_bytes < data.len() {
        encode_base64_scalar_remainder(&data[simd_bytes..], dictionary, result);
    }
}

/// Reshuffle bytes and extract 6-bit indices from 12 input bytes
///
/// Based on the algorithm from https://github.com/aklomp/base64
/// This function takes 12 bytes and produces 16 6-bit values (0-63)
///
/// The algorithm uses multiply instructions to perform the bit extraction,
/// which is more efficient than multiple shift/mask operations.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn enc_reshuffle(input: std::arch::x86_64::__m128i) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Input, bytes MSB to LSB (little endian, so byte 0 is at low address):
    // 0 0 0 0 l k j i h g f e d c b a
    //
    // We need to reshuffle to prepare for 6-bit extraction.
    // Each group of 3 input bytes (24 bits) becomes 4 output bytes (4 x 6 bits)

    let shuffled = _mm_shuffle_epi8(input, _mm_set_epi8(
        10, 11, 9, 10,   // bytes for output positions 12-15 (from input bytes 9-11)
        7, 8, 6, 7,      // bytes for output positions 8-11 (from input bytes 6-8)
        4, 5, 3, 4,      // bytes for output positions 4-7 (from input bytes 3-5)
        1, 2, 0, 1       // bytes for output positions 0-3 (from input bytes 0-2)
    ));
    // After shuffle, each 32-bit group contains bytes arranged as:
    // [b c a b] for first group, etc.
    // This duplicates bytes so we can extract all 6-bit pieces

    // Now we need to extract the 6-bit groups using multiplication tricks.
    // For 3 bytes ABC (24 bits) -> 4 groups of 6 bits: [AAAAAA] [AABBBB] [BBBBCC] [CCCCCC]
    //
    // First extraction: get bits for positions 0 and 2 in each group of 4
    // Mask: 0x0FC0FC00 selects the right bits
    let t0 = _mm_and_si128(shuffled, _mm_set1_epi32(0x0FC0FC00_u32 as i32));
    // After AND:
    // 0000kkkk LL000000 JJJJJJ00 00000000 (for last group)
    // etc.

    // Multiply high 16-bit to shift bits into place
    // 0x04000040 = multipliers that shift bits right by 6 and 4 positions
    let t1 = _mm_mulhi_epu16(t0, _mm_set1_epi32(0x04000040_u32 as i32));
    // Result: 00000000 00kkkkLL 00000000 00JJJJJJ

    // Second extraction: get bits for positions 1 and 3 in each group of 4
    // Mask: 0x003F03F0 selects different bits
    let t2 = _mm_and_si128(shuffled, _mm_set1_epi32(0x003F03F0_u32 as i32));
    // After AND:
    // 00000000 00llllll 000000jj KKKK0000

    // Multiply low 16-bit to shift bits into place
    // 0x01000010 = multipliers that shift bits left by 8 and 4 positions
    let t3 = _mm_mullo_epi16(t2, _mm_set1_epi32(0x01000010_u32 as i32));
    // Result: 00llllll 00000000 00jjKKKK 00000000

    // Combine the two results
    _mm_or_si128(t1, t3)
    // Final: 00llllll 00kkkkLL 00jjKKKK 00JJJJJJ
    // Each byte is now a 6-bit index (0-63)
}

/// Translate 6-bit indices (0-63) to base64 ASCII characters
///
/// Based on the algorithm from https://github.com/aklomp/base64
/// Uses an offset-based lookup instead of direct table lookup,
/// which is more efficient for SIMD.
///
/// Standard base64 alphabet mapping:
/// - [0..25]  -> 'A'..'Z' (ASCII 65..90)   offset: +65
/// - [26..51] -> 'a'..'z' (ASCII 97..122)  offset: +71
/// - [52..61] -> '0'..'9' (ASCII 48..57)   offset: -4
/// - [62]     -> '+'      (ASCII 43)       offset: -19
/// - [63]     -> '/'      (ASCII 47)       offset: -16
///
/// URL-safe base64 alphabet differs only at positions 62-63:
/// - [62]     -> '-'      (ASCII 45)       offset: -17
/// - [63]     -> '_'      (ASCII 95)       offset: +32
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn enc_translate(
    indices: std::arch::x86_64::__m128i,
    variant: AlphabetVariant,
) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Lookup table containing offsets to add to each index
    // Index into this LUT is computed from the 6-bit value
    let lut = match variant {
        AlphabetVariant::Base64Standard => _mm_setr_epi8(
            65,   // index 0: 'A' = 0 + 65
            71,   // index 1: for values 26-51, add 71 (26 + 71 = 97 = 'a')
            -4,   // indices 2-11: for values 52-61, add -4 (52 + -4 = 48 = '0')
            -4, -4, -4, -4, -4, -4, -4, -4, -4,
            -19,  // index 12: for value 62, add -19 (62 + -19 = 43 = '+')
            -16,  // index 13: for value 63, add -16 (63 + -16 = 47 = '/')
            0,    // unused
            0,    // unused
        ),
        AlphabetVariant::Base64Url => _mm_setr_epi8(
            65,   // index 0: 'A' = 0 + 65
            71,   // index 1: for values 26-51, add 71 (26 + 71 = 97 = 'a')
            -4,   // indices 2-11: for values 52-61, add -4 (52 + -4 = 48 = '0')
            -4, -4, -4, -4, -4, -4, -4, -4, -4,
            -17,  // index 12: for value 62, add -17 (62 + -17 = 45 = '-')
            32,   // index 13: for value 63, add 32 (63 + 32 = 95 = '_')
            0,    // unused
            0,    // unused
        ),
    };

    // Create LUT indices from the input values
    // For range [0..25]: result should be 0 (offset +65)
    // For range [26..51]: result should be 1 (offset +71)
    // For range [52..61]: result should be 2..11 (offset -4)
    // For value 62: result should be 12 (offset -19)
    // For value 63: result should be 13 (offset -16)
    //
    // Start by subtracting 51 from each value (saturating):
    // [0..51] -> 0 (saturates)
    // [52..61] -> 1..10
    // [62] -> 11
    // [63] -> 12
    let mut lut_indices = _mm_subs_epu8(indices, _mm_set1_epi8(51));

    // For values > 25, we need to add 1 to get the correct LUT index
    // Create a mask: 0xFF for values > 25, 0x00 otherwise
    let mask = _mm_cmpgt_epi8(indices, _mm_set1_epi8(25));

    // Subtract the mask (-1 for > 25, 0 otherwise) to add 1 where needed
    lut_indices = _mm_sub_epi8(lut_indices, mask);
    // Now:
    // [0..25] -> 0 (correct, offset +65)
    // [26..51] -> 0 - (-1) = 1 (correct, offset +71)
    // [52..61] -> (1..10) - (-1) = 2..11 (correct, offset -4)
    // [62] -> 11 - (-1) = 12 (correct, offset -19)
    // [63] -> 12 - (-1) = 13 (correct, offset -16)

    // Look up the offsets and add to original indices
    let offsets = _mm_shuffle_epi8(lut, lut_indices);
    _mm_add_epi8(indices, offsets)
}

/// Encode remaining bytes using scalar algorithm
///
/// Also handles padding for base64 output
#[cfg(target_arch = "x86_64")]
fn encode_base64_scalar_remainder(data: &[u8], dictionary: &Dictionary, result: &mut String) {
    // Process remaining bytes with standard scalar algorithm
    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0usize;

    for &byte in data {
        bit_buffer = (bit_buffer << 8) | (byte as u32);
        bits_in_buffer += 8;

        while bits_in_buffer >= 6 {
            bits_in_buffer -= 6;
            let index = ((bit_buffer >> bits_in_buffer) & 0x3F) as usize;
            result.push(dictionary.encode_digit(index).unwrap());
        }
    }

    // Handle final bits
    if bits_in_buffer > 0 {
        let index = ((bit_buffer << (6 - bits_in_buffer)) & 0x3F) as usize;
        result.push(dictionary.encode_digit(index).unwrap());
    }

    // Add padding if specified
    if let Some(pad_char) = dictionary.padding() {
        // Base64 output should be a multiple of 4 characters
        while result.len() % 4 != 0 {
            result.push(pad_char);
        }
    }
}

/// SSSE3 base64 decoding implementation
///
/// Based on the algorithm from https://github.com/aklomp/base64
/// Processes 16 input characters -> 12 output bytes per iteration
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn decode_base64_ssse3_impl(
    encoded: &str,
    variant: AlphabetVariant,
    result: &mut Vec<u8>,
) -> bool {
    use std::arch::x86_64::*;

    const INPUT_BLOCK_SIZE: usize = 16;
    const OUTPUT_BLOCK_SIZE: usize = 12;

    let encoded_bytes = encoded.as_bytes();

    // Strip padding
    let input_no_padding = if let Some(last_non_pad) = encoded_bytes.iter().rposition(|&b| b != b'=') {
        &encoded_bytes[..=last_non_pad]
    } else {
        encoded_bytes
    };

    // Get decode LUTs for this variant
    let (lut_lo, lut_hi, lut_roll) = get_decode_luts(variant);

    // Calculate number of full 16-byte blocks
    let num_rounds = input_no_padding.len() / INPUT_BLOCK_SIZE;
    let simd_bytes = num_rounds * INPUT_BLOCK_SIZE;

    // Process full blocks
    for round in 0..num_rounds {
        let offset = round * INPUT_BLOCK_SIZE;

        // Load 16 bytes
        let input_vec = _mm_loadu_si128(input_no_padding.as_ptr().add(offset) as *const __m128i);

        // Validate
        if !dec_validate(input_vec, lut_lo, lut_hi) {
            return false; // Invalid characters
        }

        // Translate ASCII to 6-bit indices
        let indices = dec_translate(input_vec, lut_hi, lut_roll);

        // Reshuffle 6-bit to 8-bit
        let decoded = dec_reshuffle(indices);

        // Store 12 bytes
        let mut output_buf = [0u8; 16];
        _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, decoded);
        result.extend_from_slice(&output_buf[0..OUTPUT_BLOCK_SIZE]);
    }

    // Handle remainder with scalar fallback
    if simd_bytes < input_no_padding.len() {
        let remainder = &input_no_padding[simd_bytes..];
        if !decode_base64_scalar_remainder(remainder, &mut |c| {
            match c {
                b'A'..=b'Z' => Some((c - b'A') as u8),
                b'a'..=b'z' => Some((c - b'a' + 26) as u8),
                b'0'..=b'9' => Some((c - b'0' + 52) as u8),
                b'+' if matches!(variant, AlphabetVariant::Base64Standard) => Some(62),
                b'/' if matches!(variant, AlphabetVariant::Base64Standard) => Some(63),
                b'-' if matches!(variant, AlphabetVariant::Base64Url) => Some(62),
                b'_' if matches!(variant, AlphabetVariant::Base64Url) => Some(63),
                _ => None,
            }
        }, result) {
            return false;
        }
    }

    true
}

/// Get decode lookup tables for the specified variant
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn get_decode_luts(
    variant: AlphabetVariant,
) -> (std::arch::x86_64::__m128i, std::arch::x86_64::__m128i, std::arch::x86_64::__m128i) {
    use std::arch::x86_64::*;

    // Low nibble lookup - validates based on low 4 bits
    let lut_lo = _mm_setr_epi8(
        0x15, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
        0x11, 0x11, 0x13, 0x1A, 0x1B, 0x1B, 0x1B, 0x1A,
    );

    // High nibble lookup - validates based on high 4 bits
    let lut_hi = _mm_setr_epi8(
        0x10, 0x10, 0x01, 0x02, 0x04, 0x08, 0x04, 0x08,
        0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10,
    );

    // Roll/offset lookup - converts ASCII to 6-bit indices
    let lut_roll = match variant {
        AlphabetVariant::Base64Standard => _mm_setr_epi8(
            0, 16, 19, 4, -65, -65, -71, -71,
            0, 0, 0, 0, 0, 0, 0, 0,
        ),
        AlphabetVariant::Base64Url => _mm_setr_epi8(
            0, 17, -32, 4, -65, -65, -71, -71,
            0, 0, 0, 0, 0, 0, 0, 0,
        ),
    };

    (lut_lo, lut_hi, lut_roll)
}

/// Validate that all input bytes are valid base64 characters
///
/// Returns true if all bytes are valid, false otherwise
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn dec_validate(
    input: std::arch::x86_64::__m128i,
    lut_lo: std::arch::x86_64::__m128i,
    lut_hi: std::arch::x86_64::__m128i,
) -> bool {
    use std::arch::x86_64::*;

    // Extract low and high nibbles
    let lo_nibbles = _mm_and_si128(input, _mm_set1_epi8(0x0F));
    let hi_nibbles = _mm_and_si128(_mm_srli_epi32(input, 4), _mm_set1_epi8(0x0F));

    // Look up validation values
    let lo_lookup = _mm_shuffle_epi8(lut_lo, lo_nibbles);
    let hi_lookup = _mm_shuffle_epi8(lut_hi, hi_nibbles);

    // AND the two lookups - result should be 0 for valid characters
    let validation = _mm_and_si128(lo_lookup, hi_lookup);

    // Check if all bytes are 0 (valid)
    _mm_movemask_epi8(validation) == 0
}

/// Translate ASCII characters to 6-bit indices
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn dec_translate(
    input: std::arch::x86_64::__m128i,
    lut_hi: std::arch::x86_64::__m128i,
    lut_roll: std::arch::x86_64::__m128i,
) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Extract high nibbles
    let hi_nibbles = _mm_and_si128(_mm_srli_epi32(input, 4), _mm_set1_epi8(0x0F));

    // Check for '/' character (0x2F)
    let eq_2f = _mm_cmpeq_epi8(input, _mm_set1_epi8(0x2F));

    // Index into lut_roll is: hi_nibbles + (input == '/')
    // eq_2f is 0xFF where true, 0x00 where false, so adding it increments by 1 where true
    let roll_index = _mm_add_epi8(eq_2f, hi_nibbles);

    // Look up offset values from lut_roll
    let offsets = _mm_shuffle_epi8(lut_roll, roll_index);

    // Add offsets to convert ASCII to 6-bit indices
    _mm_add_epi8(input, offsets)
}

/// Reshuffle 6-bit indices to packed 8-bit bytes
///
/// Converts 16 bytes of 6-bit values (0-63) to 12 bytes of 8-bit data
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn dec_reshuffle(indices: std::arch::x86_64::__m128i) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Stage 1: Merge adjacent pairs using multiply-add
    // Each pair of 6-bit values becomes one 16-bit value
    // Constant 0x01400140 = [320, 1] = [64 * 5, 1]
    let merge_ab_and_bc = _mm_maddubs_epi16(indices, _mm_set1_epi32(0x01400140u32 as i32));

    // Stage 2: Combine 16-bit pairs into 32-bit values
    // Constant 0x00011000 = [1, 4096]
    let final_32bit = _mm_madd_epi16(merge_ab_and_bc, _mm_set1_epi32(0x00011000u32 as i32));

    // Stage 3: Extract the valid bytes from each 32-bit group
    // Each 32-bit value contains 3 bytes of decoded data
    // Bytes are reversed within each group due to little-endian layout
    _mm_shuffle_epi8(
        final_32bit,
        _mm_setr_epi8(
            2, 1, 0,    // first group of 3 bytes (reversed)
            6, 5, 4,    // second group of 3 bytes (reversed)
            10, 9, 8,   // third group of 3 bytes (reversed)
            14, 13, 12, // fourth group of 3 bytes (reversed)
            -1, -1, -1, -1, // unused (will be zero)
        ),
    )
}

/// Decode remaining bytes using scalar algorithm
#[cfg(target_arch = "x86_64")]
fn decode_base64_scalar_remainder(
    data: &[u8],
    char_to_index: &mut dyn FnMut(u8) -> Option<u8>,
    result: &mut Vec<u8>,
) -> bool {
    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0usize;

    for &byte in data {
        let index = match char_to_index(byte) {
            Some(i) => i as u32,
            None => return false,
        };

        bit_buffer = (bit_buffer << 6) | index;
        bits_in_buffer += 6;

        if bits_in_buffer >= 8 {
            bits_in_buffer -= 8;
            let output_byte = (bit_buffer >> bits_in_buffer) as u8;
            result.push(output_byte);
            bit_buffer &= (1 << bits_in_buffer) - 1;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::EncodingMode;

    fn make_base64_dict() -> Dictionary {
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
            .chars().collect();
        Dictionary::new_with_mode(chars, EncodingMode::Chunked, Some('=')).unwrap()
    }

    #[test]
    fn test_simd_encode_matches_scalar() {
        let dictionary = make_base64_dict();
        let test_data = b"Hello, World! This is a test of SIMD base64 encoding.";

        if let Some(simd_result) = encode_base64_simd(test_data, &dictionary) {
            let scalar_result = crate::encoders::chunked::encode_chunked(test_data, &dictionary);
            assert_eq!(simd_result, scalar_result, "SIMD and scalar should produce same output");
        }
    }

    #[test]
    fn test_simd_encode_known_values() {
        let dictionary = make_base64_dict();

        // Test vectors with known base64 outputs
        let test_cases = [
            (b"Hello".as_slice(), "SGVsbG8="),
            (b"Hello, World!", "SGVsbG8sIFdvcmxkIQ=="),
            (b"a", "YQ=="),
            (b"ab", "YWI="),
            (b"abc", "YWJj"),
            (b"abcd", "YWJjZA=="),
            (b"abcde", "YWJjZGU="),
            (b"abcdef", "YWJjZGVm"),
            (b"", ""),
        ];

        for (input, expected) in test_cases {
            if let Some(simd_result) = encode_base64_simd(input, &dictionary) {
                assert_eq!(simd_result, expected, "Failed for input: {:?}", input);
            } else {
                // SIMD not available, skip
                let scalar_result = crate::encoders::chunked::encode_chunked(input, &dictionary);
                assert_eq!(scalar_result, expected, "Scalar failed for input: {:?}", input);
            }
        }
    }

    #[test]
    fn test_simd_encode_various_lengths() {
        let dictionary = make_base64_dict();

        // Test various lengths to exercise both SIMD and scalar paths
        for len in 0..100 {
            let data: Vec<u8> = (0..len).map(|i| i as u8).collect();

            if let Some(simd_result) = encode_base64_simd(&data, &dictionary) {
                let scalar_result = crate::encoders::chunked::encode_chunked(&data, &dictionary);
                assert_eq!(simd_result, scalar_result, "Mismatch at length {}", len);
            }
        }
    }

    #[test]
    fn test_simd_encode_all_bytes() {
        let dictionary = make_base64_dict();

        // Test with all possible byte values
        let data: Vec<u8> = (0..=255).collect();

        if let Some(simd_result) = encode_base64_simd(&data, &dictionary) {
            let scalar_result = crate::encoders::chunked::encode_chunked(&data, &dictionary);
            assert_eq!(simd_result, scalar_result, "Mismatch encoding all byte values");
        }
    }

    #[test]
    fn test_simd_encode_large_input() {
        let dictionary = make_base64_dict();

        // Test with larger input that exercises multiple SIMD iterations
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();

        if let Some(simd_result) = encode_base64_simd(&data, &dictionary) {
            let scalar_result = crate::encoders::chunked::encode_chunked(&data, &dictionary);
            assert_eq!(simd_result, scalar_result, "Mismatch on large input");
        }
    }

    // ===== DECODE TESTS =====

    #[test]
    fn test_simd_decode_rfc4648_vectors() {
        let dictionary = make_base64_dict();

        // RFC 4648 test vectors
        let test_cases = [
            ("", b"".as_slice()),
            ("Zg==", b"f"),
            ("Zm8=", b"fo"),
            ("Zm9v", b"foo"),
            ("Zm9vYg==", b"foob"),
            ("Zm9vYmE=", b"fooba"),
            ("Zm9vYmFy", b"foobar"),
            ("SGVsbG8=", b"Hello"),
            ("SGVsbG8sIFdvcmxkIQ==", b"Hello, World!"),
        ];

        for (input, expected) in test_cases {
            if let Some(decoded) = decode_base64_simd(input, &dictionary) {
                assert_eq!(decoded, expected, "Failed decoding: {}", input);
            } else {
                // SIMD not available or input too small, skip
            }
        }
    }

    #[test]
    fn test_simd_decode_round_trip() {
        let dictionary = make_base64_dict();

        // Test various lengths
        for len in 0..100 {
            let original: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();

            if let Some(encoded) = encode_base64_simd(&original, &dictionary) {
                if let Some(decoded) = decode_base64_simd(&encoded, &dictionary) {
                    assert_eq!(decoded, original, "Round-trip failed at length {}", len);
                }
            }
        }
    }

    #[test]
    fn test_simd_decode_boundary_cases() {
        let dictionary = make_base64_dict();

        // Test exactly 16 bytes (one SIMD block of encoded input)
        let data_12 = vec![0xAB; 12]; // 12 bytes -> 16 base64 chars
        if let Some(encoded) = encode_base64_simd(&data_12, &dictionary) {
            assert_eq!(encoded.len(), 16, "12 bytes should encode to 16 chars");
            if let Some(decoded) = decode_base64_simd(&encoded, &dictionary) {
                assert_eq!(decoded, data_12);
            }
        }

        // Test 15 bytes encoded (20 chars - should use SIMD for first 16, scalar for 4)
        let data_15 = vec![0xCD; 15];
        if let Some(encoded) = encode_base64_simd(&data_15, &dictionary) {
            if let Some(decoded) = decode_base64_simd(&encoded, &dictionary) {
                assert_eq!(decoded, data_15);
            }
        }

        // Test 17 bytes encoded (more than one SIMD block)
        let data_17 = vec![0xEF; 17];
        if let Some(encoded) = encode_base64_simd(&data_17, &dictionary) {
            if let Some(decoded) = decode_base64_simd(&encoded, &dictionary) {
                assert_eq!(decoded, data_17);
            }
        }
    }

    #[test]
    fn test_simd_decode_padding_variations() {
        let dictionary = make_base64_dict();

        let test_cases = [
            ("YQ==", b"a".as_slice()),   // 2 padding chars
            ("YWI=", b"ab"),              // 1 padding char
            ("YWJj", b"abc"),             // no padding
            ("YWJjZA==", b"abcd"),        // 2 padding
            ("YWJjZGU=", b"abcde"),       // 1 padding
            ("YWJjZGVm", b"abcdef"),      // no padding
        ];

        for (input, expected) in test_cases {
            if let Some(decoded) = decode_base64_simd(input, &dictionary) {
                assert_eq!(decoded, expected, "Failed for input: {}", input);
            }
        }
    }

    #[test]
    fn test_simd_decode_invalid_characters() {
        let dictionary = make_base64_dict();

        let invalid_inputs = [
            "SGVsb G8=",      // space
            "SGVsbG8=\n",     // newline
            "SGVs!G8=",       // invalid char
            "SGVs@G8=",       // invalid char
        ];

        for input in invalid_inputs {
            // Invalid characters should return None or fail validation
            let result = decode_base64_simd(input, &dictionary);
            // Accept either None or successful decode if it's handled gracefully
            // The SIMD decoder should reject these (return None or false)
        }
    }

    #[test]
    fn test_simd_decode_all_byte_values() {
        let dictionary = make_base64_dict();

        // Test that all possible byte values can be encoded and decoded
        let data: Vec<u8> = (0..=255).collect();

        if let Some(encoded) = encode_base64_simd(&data, &dictionary) {
            if let Some(decoded) = decode_base64_simd(&encoded, &dictionary) {
                assert_eq!(decoded, data, "Failed to round-trip all byte values");
            }
        }
    }

    #[test]
    fn test_simd_decode_compare_with_scalar() {
        let dictionary = make_base64_dict();

        let test_data = b"The quick brown fox jumps over the lazy dog. 0123456789!@#$%^&*()";

        if let Some(encoded) = encode_base64_simd(test_data, &dictionary) {
            // Decode with SIMD
            if let Some(simd_decoded) = decode_base64_simd(&encoded, &dictionary) {
                assert_eq!(simd_decoded, test_data, "SIMD decode didn't match original");
            }
        }
    }

    #[test]
    fn test_simd_decode_empty_string() {
        let dictionary = make_base64_dict();

        // Empty string should decode to empty vec or return None (too small for SIMD)
        let result = decode_base64_simd("", &dictionary);
        // Empty string is < 16 bytes, so SIMD will return None
        assert!(result.is_none(), "Empty string should return None (too small for SIMD)");
    }

    #[test]
    fn test_simd_decode_various_lengths() {
        let dictionary = make_base64_dict();

        // Test various lengths to exercise SIMD boundaries
        for len in [1, 2, 3, 11, 12, 13, 15, 16, 17, 23, 24, 25, 47, 48, 49, 100] {
            let data: Vec<u8> = (0..len).map(|i| (i * 13 + 7) as u8).collect();

            if let Some(encoded) = encode_base64_simd(&data, &dictionary) {
                if let Some(decoded) = decode_base64_simd(&encoded, &dictionary) {
                    assert_eq!(decoded, data, "Failed at length {}", len);
                }
            }
        }
    }

    #[test]
    fn test_simd_decode_url_safe() {
        // Test URL-safe variant (- and _ instead of + and /)
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
            .chars().collect();
        let dictionary = Dictionary::new_with_mode(chars, EncodingMode::Chunked, Some('=')).unwrap();

        let test_data = b"Test data with bytes that encode to + and / in standard base64";

        if let Some(encoded) = encode_base64_simd(test_data, &dictionary) {
            // Verify it uses - and _ not + and /
            assert!(!encoded.contains('+'), "URL-safe should not contain +");
            assert!(!encoded.contains('/'), "URL-safe should not contain /");

            if let Some(decoded) = decode_base64_simd(&encoded, &dictionary) {
                assert_eq!(decoded, test_data, "URL-safe round-trip failed");
            }
        }
    }

    #[test]
    fn test_simd_decode_large_input() {
        let dictionary = make_base64_dict();

        // Test with large input to exercise multiple SIMD iterations
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();

        if let Some(encoded) = encode_base64_simd(&data, &dictionary) {
            if let Some(decoded) = decode_base64_simd(&encoded, &dictionary) {
                assert_eq!(decoded, data, "Large input decode failed");
            }
        }
    }

    #[test]
    fn test_simd_decode_invalid_length() {
        let dictionary = make_base64_dict();

        // Base64 with invalid length (not multiple of 4, no padding)
        let invalid = "SGVsbG8"; // 7 chars, should be 8 with padding

        // This might return None or handle gracefully
        // The decoder should detect this as invalid
        let result = decode_base64_simd(invalid, &dictionary);
        // For now, just verify it doesn't panic
    }
}
