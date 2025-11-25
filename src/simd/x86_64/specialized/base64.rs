//! SIMD implementation for base64 (6-bit encoding)
//!
//! Based on techniques from:
//! - https://github.com/aklomp/base64 (reference C implementation)
//! - Wojciech Muła's SIMD base64 work
//! - Intel optimization manuals

use super::super::common;
use crate::core::dictionary::Dictionary;
use crate::simd::alphabets::AlphabetVariant;

/// SIMD-accelerated base64 encoding using SSSE3
///
/// Processes 12 bytes at a time, producing 16 base64 characters.
/// Falls back to scalar for remainder.
pub fn encode(data: &[u8], dictionary: &Dictionary, variant: AlphabetVariant) -> Option<String> {
    // Pre-allocate output
    let output_len = ((data.len() + 2) / 3) * 4;
    let mut result = String::with_capacity(output_len);

    // SAFETY: Caller verified SSSE3 support
    unsafe {
        encode_ssse3_impl(data, dictionary, variant, &mut result);
    }

    Some(result)
}

/// SIMD-accelerated base64 decoding using SSSE3
pub fn decode(encoded: &str, variant: AlphabetVariant) -> Option<Vec<u8>> {
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

    // SAFETY: Caller verified SSSE3 support
    unsafe {
        if !decode_ssse3_impl(encoded_bytes, variant, &mut result) {
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
unsafe fn encode_ssse3_impl(
    data: &[u8],
    dictionary: &Dictionary,
    variant: AlphabetVariant,
    result: &mut String,
) {
    use std::arch::x86_64::*;

    const BLOCK_SIZE: usize = 12;

    // Need at least 16 bytes in buffer to safely load 128 bits
    if data.len() < 16 {
        // Fall back to scalar for small inputs
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
        let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

        // Reshuffle bytes to extract 6-bit groups
        let reshuffled = reshuffle(input_vec);

        // Translate 6-bit indices to ASCII
        let encoded = translate(reshuffled, variant);

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
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn reshuffle(input: std::arch::x86_64::__m128i) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Input, bytes MSB to LSB (little endian, so byte 0 is at low address):
    // 0 0 0 0 l k j i h g f e d c b a
    //
    // We need to reshuffle to prepare for 6-bit extraction.
    // Each group of 3 input bytes (24 bits) becomes 4 output bytes (4 x 6 bits)

    let shuffled = _mm_shuffle_epi8(
        input,
        _mm_set_epi8(
            10, 11, 9, 10, // bytes for output positions 12-15 (from input bytes 9-11)
            7, 8, 6, 7, // bytes for output positions 8-11 (from input bytes 6-8)
            4, 5, 3, 4, // bytes for output positions 4-7 (from input bytes 3-5)
            1, 2, 0, 1, // bytes for output positions 0-3 (from input bytes 0-2)
        ),
    );

    // Now we need to extract the 6-bit groups using multiplication tricks.
    // For 3 bytes ABC (24 bits) -> 4 groups of 6 bits: [AAAAAA] [AABBBB] [BBBBCC] [CCCCCC]

    // First extraction: get bits for positions 0 and 2 in each group of 4
    let t0 = _mm_and_si128(shuffled, _mm_set1_epi32(0x0FC0FC00_u32 as i32));
    let t1 = _mm_mulhi_epu16(t0, _mm_set1_epi32(0x04000040_u32 as i32));

    // Second extraction: get bits for positions 1 and 3 in each group of 4
    let t2 = _mm_and_si128(shuffled, _mm_set1_epi32(0x003F03F0_u32 as i32));
    let t3 = _mm_mullo_epi16(t2, _mm_set1_epi32(0x01000010_u32 as i32));

    // Combine the two results
    _mm_or_si128(t1, t3)
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
unsafe fn translate(
    indices: std::arch::x86_64::__m128i,
    variant: AlphabetVariant,
) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Lookup table containing offsets to add to each index
    let lut = match variant {
        AlphabetVariant::Base64Standard => _mm_setr_epi8(
            65, // index 0: 'A' = 0 + 65
            71, // index 1: for values 26-51, add 71 (26 + 71 = 97 = 'a')
            -4, // indices 2-11: for values 52-61, add -4 (52 + -4 = 48 = '0')
            -4, -4, -4, -4, -4, -4, -4, -4, -4,
            -19, // index 12: for value 62, add -19 (62 + -19 = 43 = '+')
            -16, // index 13: for value 63, add -16 (63 + -16 = 47 = '/')
            0,   // unused
            0,   // unused
        ),
        AlphabetVariant::Base64Url => _mm_setr_epi8(
            65, // index 0: 'A' = 0 + 65
            71, // index 1: for values 26-51, add 71 (26 + 71 = 97 = 'a')
            -4, // indices 2-11: for values 52-61, add -4 (52 + -4 = 48 = '0')
            -4, -4, -4, -4, -4, -4, -4, -4, -4,
            -17, // index 12: for value 62, add -17 (62 + -17 = 45 = '-')
            32,  // index 13: for value 63, add 32 (63 + 32 = 95 = '_')
            0,   // unused
            0,   // unused
        ),
    };

    // Create LUT indices from the input values
    let mut lut_indices = _mm_subs_epu8(indices, _mm_set1_epi8(51));
    let mask = _mm_cmpgt_epi8(indices, _mm_set1_epi8(25));
    lut_indices = _mm_sub_epi8(lut_indices, mask);

    // Look up the offsets and add to original indices
    let offsets = _mm_shuffle_epi8(lut, lut_indices);
    _mm_add_epi8(indices, offsets)
}

/// Encode remaining bytes using scalar algorithm
///
/// Also handles padding for base64 output
fn encode_scalar_remainder(data: &[u8], dictionary: &Dictionary, result: &mut String) {
    // Use common scalar chunked encoding (6-bit for base64)
    common::encode_scalar_chunked(data, dictionary, result);
}

/// SSSE3 base64 decoding implementation
///
/// Based on the algorithm from https://github.com/aklomp/base64
/// Processes 16 input characters -> 12 output bytes per iteration
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn decode_ssse3_impl(
    encoded: &[u8],
    variant: AlphabetVariant,
    result: &mut Vec<u8>,
) -> bool {
    use std::arch::x86_64::*;

    const INPUT_BLOCK_SIZE: usize = 16;
    const OUTPUT_BLOCK_SIZE: usize = 12;

    // Strip padding
    let input_no_padding = if let Some(last_non_pad) = encoded.iter().rposition(|&b| b != b'=') {
        &encoded[..=last_non_pad]
    } else {
        encoded
    };

    // Get decode LUTs for this variant
    let (lut_lo, lut_hi, lut_roll) = get_decode_luts(variant);

    // Calculate number of full 16-byte blocks
    let (num_rounds, simd_bytes) =
        common::calculate_blocks(input_no_padding.len(), INPUT_BLOCK_SIZE);

    // Process full blocks
    for round in 0..num_rounds {
        let offset = round * INPUT_BLOCK_SIZE;

        // Load 16 bytes
        let input_vec = _mm_loadu_si128(input_no_padding.as_ptr().add(offset) as *const __m128i);

        // Validate
        if !validate(input_vec, lut_lo, lut_hi) {
            return false; // Invalid characters
        }

        // Translate ASCII to 6-bit indices
        let indices = translate_decode(input_vec, lut_hi, lut_roll);

        // Reshuffle 6-bit to 8-bit
        let decoded = reshuffle_decode(indices);

        // Store 12 bytes
        let mut output_buf = [0u8; 16];
        _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, decoded);
        result.extend_from_slice(&output_buf[0..OUTPUT_BLOCK_SIZE]);
    }

    // Handle remainder with scalar fallback
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

/// Get decode lookup tables for the specified variant
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn get_decode_luts(
    variant: AlphabetVariant,
) -> (
    std::arch::x86_64::__m128i,
    std::arch::x86_64::__m128i,
    std::arch::x86_64::__m128i,
) {
    use std::arch::x86_64::*;

    // Low nibble lookup - validates based on low 4 bits
    let lut_lo = _mm_setr_epi8(
        0x15, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x13, 0x1A, 0x1B, 0x1B, 0x1B,
        0x1A,
    );

    // High nibble lookup - validates based on high 4 bits
    let lut_hi = _mm_setr_epi8(
        0x10, 0x10, 0x01, 0x02, 0x04, 0x08, 0x04, 0x08, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10,
        0x10,
    );

    // Roll/offset lookup - converts ASCII to 6-bit indices
    let lut_roll = match variant {
        AlphabetVariant::Base64Standard => {
            _mm_setr_epi8(0, 16, 19, 4, -65, -65, -71, -71, 0, 0, 0, 0, 0, 0, 0, 0)
        }
        AlphabetVariant::Base64Url => {
            _mm_setr_epi8(0, 17, -32, 4, -65, -65, -71, -71, 0, 0, 0, 0, 0, 0, 0, 0)
        }
    };

    (lut_lo, lut_hi, lut_roll)
}

/// Validate that all input bytes are valid base64 characters
///
/// Returns true if all bytes are valid, false otherwise
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn validate(
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
unsafe fn translate_decode(
    input: std::arch::x86_64::__m128i,
    _lut_hi: std::arch::x86_64::__m128i,
    lut_roll: std::arch::x86_64::__m128i,
) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Extract high nibbles
    let hi_nibbles = _mm_and_si128(_mm_srli_epi32(input, 4), _mm_set1_epi8(0x0F));

    // Check for '/' character (0x2F)
    let eq_2f = _mm_cmpeq_epi8(input, _mm_set1_epi8(0x2F));

    // Index into lut_roll is: hi_nibbles + (input == '/')
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
unsafe fn reshuffle_decode(indices: std::arch::x86_64::__m128i) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Stage 1: Merge adjacent pairs using multiply-add
    let merge_ab_and_bc = _mm_maddubs_epi16(indices, _mm_set1_epi32(0x01400140u32 as i32));

    // Stage 2: Combine 16-bit pairs into 32-bit values
    let final_32bit = _mm_madd_epi16(merge_ab_and_bc, _mm_set1_epi32(0x00011000u32 as i32));

    // Stage 3: Extract the valid bytes from each 32-bit group
    _mm_shuffle_epi8(
        final_32bit,
        _mm_setr_epi8(
            2, 1, 0, // first group of 3 bytes (reversed)
            6, 5, 4, // second group of 3 bytes (reversed)
            10, 9, 8, // third group of 3 bytes (reversed)
            14, 13, 12, // fourth group of 3 bytes (reversed)
            -1, -1, -1, -1, // unused
        ),
    )
}

/// Decode remaining bytes using scalar algorithm
fn decode_scalar_remainder(
    data: &[u8],
    char_to_index: &mut dyn FnMut(u8) -> Option<u8>,
    result: &mut Vec<u8>,
) -> bool {
    // Use common scalar chunked decoding (6-bit for base64)
    common::decode_scalar_chunked(data, char_to_index, result, 6)
}

#[cfg(test)]
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

    #[test]
    fn test_encode_matches_scalar() {
        let dictionary = make_base64_dict();
        let test_data = b"Hello, World! This is a test of SIMD base64 encoding.";

        if let Some(simd_result) = encode(test_data, &dictionary, AlphabetVariant::Base64Standard) {
            let scalar_result = crate::encoders::chunked::encode_chunked(test_data, &dictionary);
            assert_eq!(
                simd_result, scalar_result,
                "SIMD and scalar should produce same output"
            );
        }
    }

    #[test]
    fn test_encode_known_values() {
        let dictionary = make_base64_dict();

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
            if let Some(simd_result) = encode(input, &dictionary, AlphabetVariant::Base64Standard) {
                assert_eq!(simd_result, expected, "Failed for input: {:?}", input);
            }
        }
    }

    #[test]
    fn test_decode_round_trip() {
        let dictionary = make_base64_dict();

        for len in 0..100 {
            let original: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();

            if let Some(encoded) = encode(&original, &dictionary, AlphabetVariant::Base64Standard) {
                if let Some(decoded) = decode(&encoded, AlphabetVariant::Base64Standard) {
                    assert_eq!(decoded, original, "Round-trip failed at length {}", len);
                }
            }
        }
    }
}
