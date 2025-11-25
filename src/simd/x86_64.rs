//! x86_64 SIMD implementations using AVX2 and SSSE3
//!
//! Based on techniques from:
//! - https://github.com/aklomp/base64 (reference C implementation)
//! - Wojciech Muła's SIMD base64 work
//! - Intel optimization manuals

use crate::core::dictionary::Dictionary;

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

    // Verify this is a standard base64 dictionary
    if !is_standard_base64(dictionary) {
        return None;
    }

    // Need SSSE3 for pshufb
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Pre-allocate output
    let output_len = ((data.len() + 2) / 3) * 4;
    let mut result = String::with_capacity(output_len);

    // SAFETY: We checked for SSSE3 support above
    unsafe {
        encode_base64_ssse3_impl(data, dictionary, &mut result);
    }

    Some(result)
}

/// SIMD-accelerated base64 decoding using AVX2
#[cfg(target_arch = "x86_64")]
pub fn decode_base64_simd(_encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize standard base64
    if dictionary.base() != 64 || !is_standard_base64(dictionary) {
        return None;
    }

    // For now, return None to use scalar path
    // Full SIMD decoding is more complex and will be implemented separately
    None
}

#[cfg(target_arch = "x86_64")]
fn is_standard_base64(dictionary: &Dictionary) -> bool {
    // Check if this is standard base64 dictionary
    const STANDARD_B64: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    for (i, expected) in STANDARD_B64.chars().enumerate() {
        if dictionary.encode_digit(i) != Some(expected) {
            return false;
        }
    }
    true
}

/// SSSE3 base64 encoding implementation
///
/// Based on the algorithm from https://github.com/aklomp/base64
/// Processes 12 input bytes -> 16 output characters per iteration
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn encode_base64_ssse3_impl(data: &[u8], dictionary: &Dictionary, result: &mut String) {
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
        let encoded = enc_translate(reshuffled);

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
/// The base64 alphabet maps as:
/// - [0..25]  -> 'A'..'Z' (ASCII 65..90)   offset: +65
/// - [26..51] -> 'a'..'z' (ASCII 97..122)  offset: +71
/// - [52..61] -> '0'..'9' (ASCII 48..57)   offset: -4
/// - [62]     -> '+'      (ASCII 43)       offset: -19
/// - [63]     -> '/'      (ASCII 47)       offset: -16
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn enc_translate(indices: std::arch::x86_64::__m128i) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Lookup table containing offsets to add to each index
    // Index into this LUT is computed from the 6-bit value
    let lut = _mm_setr_epi8(
        65,   // index 0: 'A' = 0 + 65
        71,   // index 1: for values 26-51, add 71 (26 + 71 = 97 = 'a')
        -4,   // indices 2-11: for values 52-61, add -4 (52 + -4 = 48 = '0')
        -4,
        -4, -4, -4, -4,
        -4, -4, -4, -4,
        -19,  // index 12: for value 62, add -19 (62 + -19 = 43 = '+')
        -16,  // index 13: for value 63, add -16 (63 + -16 = 47 = '/')
        0,    // unused
        0     // unused
    );

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
}
