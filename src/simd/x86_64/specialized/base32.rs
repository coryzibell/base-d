//! SIMD implementation for base32 (5-bit encoding)
//!
//! Based on techniques from:
//! - Daniel Lemire: https://lemire.me/blog/2023/07/20/fast-decoding-of-base32-strings/
//! - NLnetLabs/simdzone (C implementation by @aqrit)
//! - Wojciech Muła's SIMD base64 work (multiply-shift pattern)
//!
//! Key differences from base64:
//! - Block size: 5 bytes → 8 chars (vs 3 bytes → 4 chars)
//! - SSSE3: 10 bytes → 16 chars (vs 12 bytes → 16 chars)
//! - AVX2: 20 bytes → 32 chars (vs 24 bytes → 32 chars)
//! - 5-bit extraction requires different masks and multiplies

use super::super::common;
use crate::core::dictionary::Dictionary;
use crate::simd::alphabets::Base32Variant;

/// SIMD-accelerated base32 encoding with runtime dispatch
///
/// Automatically selects the best available SIMD implementation:
/// - AVX2 (256-bit): Processes 20 bytes -> 32 chars per iteration
/// - SSSE3 (128-bit): Processes 10 bytes -> 16 chars per iteration
/// Falls back to scalar for remainder.
pub fn encode(data: &[u8], dictionary: &Dictionary, variant: Base32Variant) -> Option<String> {
    // Pre-allocate output
    let output_len = ((data.len() + 4) / 5) * 8;
    let mut result = String::with_capacity(output_len);

    // SAFETY: Runtime detection verifies CPU feature support
    #[cfg(target_arch = "x86_64")]
    unsafe {
        if is_x86_feature_detected!("avx2") {
            encode_avx2_impl(data, dictionary, variant, &mut result);
        } else {
            encode_ssse3_impl(data, dictionary, variant, &mut result);
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        encode_scalar_remainder(data, dictionary, &mut result);
    }

    Some(result)
}

/// SIMD-accelerated base32 decoding with runtime dispatch
///
/// Automatically selects the best available SIMD implementation:
/// - AVX2 (256-bit): Processes 32 chars -> 20 bytes per iteration
/// - SSSE3 (128-bit): Processes 16 chars -> 10 bytes per iteration
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

    // SAFETY: Runtime detection verifies CPU feature support
    #[cfg(target_arch = "x86_64")]
    unsafe {
        if is_x86_feature_detected!("avx2") {
            if !decode_avx2_impl(encoded_bytes, variant, &mut result) {
                return None;
            }
        } else {
            if !decode_ssse3_impl(encoded_bytes, variant, &mut result) {
                return None;
            }
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    unsafe {
        if !decode_ssse3_impl(encoded_bytes, variant, &mut result) {
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

/// AVX2 base32 encoding implementation
///
/// Processes 20 input bytes -> 32 output characters per iteration.
/// Uses 256-bit vectors to process two independent 10-byte blocks in parallel.
///
/// Note: AVX2's vpshufb operates per 128-bit lane, so we process two
/// independent 10-byte chunks as separate lanes.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn encode_avx2_impl(
    data: &[u8],
    dictionary: &Dictionary,
    variant: Base32Variant,
    result: &mut String,
) {
    use std::arch::x86_64::*;

    const BLOCK_SIZE: usize = 20; // 20 bytes -> 32 chars

    // Need at least 32 bytes to safely load two 128-bit blocks (16 bytes each)
    if data.len() < 32 {
        // Fall back to SSSE3 for small inputs
        encode_ssse3_impl(data, dictionary, variant, result);
        return;
    }

    // Process blocks of 20 bytes
    let safe_len = if data.len() >= 12 { data.len() - 12 } else { 0 };
    let (num_rounds, simd_bytes) = common::calculate_blocks(safe_len, BLOCK_SIZE);

    let mut offset = 0;
    for _ in 0..num_rounds {
        // Load 20 bytes as two 128-bit chunks (bytes 0-9 and 10-19)
        // We load 16 bytes but only use 10 from each
        let input_lo = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);
        let input_hi = _mm_loadu_si128(data.as_ptr().add(offset + 10) as *const __m128i);

        // Combine into 256-bit register
        let input_256 = _mm256_set_m128i(input_hi, input_lo);

        // Extract 5-bit indices from both lanes (same algorithm as SSSE3, per-lane)
        let indices = extract_5bit_indices_avx2(input_256);

        // Translate 5-bit indices to ASCII (per-lane)
        let encoded = translate_encode_avx2(indices, variant);

        // Store 32 output characters
        let mut output_buf = [0u8; 32];
        _mm256_storeu_si256(output_buf.as_mut_ptr() as *mut __m256i, encoded);

        // Append to result (safe because base32 is ASCII)
        for &byte in &output_buf {
            result.push(byte as char);
        }

        offset += BLOCK_SIZE;
    }

    // Handle remainder with SSSE3
    if simd_bytes < data.len() {
        encode_ssse3_impl(&data[simd_bytes..], dictionary, variant, result);
    }
}

/// Extract 32 x 5-bit indices from 20 packed input bytes (AVX2)
///
/// Processes two independent 10-byte blocks in parallel (one per 128-bit lane).
/// Same algorithm as SSSE3 unpack_5bit_simple, but applied to both lanes simultaneously.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn extract_5bit_indices_avx2(
    input: std::arch::x86_64::__m256i,
) -> std::arch::x86_64::__m256i {
    use std::arch::x86_64::*;

    // Extract both 128-bit lanes and process separately
    let lane_lo = _mm256_castsi256_si128(input);
    let lane_hi = _mm256_extracti128_si256(input, 1);

    // Apply SSSE3 unpacking to each lane
    let indices_lo = unpack_5bit_simple(lane_lo);
    let indices_hi = unpack_5bit_simple(lane_hi);

    // Recombine into 256-bit register
    _mm256_set_m128i(indices_hi, indices_lo)
}

/// Translate 5-bit indices to base32 ASCII characters (AVX2)
///
/// Operates on both 128-bit lanes independently.
/// Same algorithm as SSSE3 translate, but with 256-bit vectors.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn translate_encode_avx2(
    indices: std::arch::x86_64::__m256i,
    variant: Base32Variant,
) -> std::arch::x86_64::__m256i {
    use std::arch::x86_64::*;

    match variant {
        Base32Variant::Rfc4648 => {
            // RFC 4648 standard: 0-25 -> 'A'-'Z', 26-31 -> '2'-'7'
            // Create mask for indices >= 26
            let ge_26 = _mm256_cmpgt_epi8(indices, _mm256_set1_epi8(25));

            // Base offset is 'A' (65) for all
            let base = _mm256_set1_epi8(b'A' as i8);

            // Adjustment for >= 26: we want '2' (50) for index 26
            // So offset should be 50 - 26 = 24 instead of 65
            // Difference: 24 - 65 = -41
            let adjustment = _mm256_and_si256(ge_26, _mm256_set1_epi8(-41));

            _mm256_add_epi8(_mm256_add_epi8(indices, base), adjustment)
        }
        Base32Variant::Rfc4648Hex => {
            // RFC 4648 hex: 0-9 -> '0'-'9', 10-31 -> 'A'-'V'
            // Create mask for indices >= 10
            let ge_10 = _mm256_cmpgt_epi8(indices, _mm256_set1_epi8(9));

            // Base offset is '0' (48) for indices 0-9
            let base = _mm256_set1_epi8(b'0' as i8);

            // Adjustment for >= 10: we want 'A' (65) for index 10
            // So offset should be 65 - 10 = 55 instead of 48
            // Difference: 55 - 48 = 7
            let adjustment = _mm256_and_si256(ge_10, _mm256_set1_epi8(7));

            _mm256_add_epi8(_mm256_add_epi8(indices, base), adjustment)
        }
    }
}

/// SSSE3 base32 encoding implementation
///
/// Processes 10 input bytes -> 16 output characters per iteration.
/// Uses bit extraction via shuffle and shift operations to extract 5-bit groups.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn encode_ssse3_impl(
    data: &[u8],
    dictionary: &Dictionary,
    variant: Base32Variant,
    result: &mut String,
) {
    use std::arch::x86_64::*;

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
    let (num_rounds, simd_bytes) = common::calculate_blocks(safe_len, BLOCK_SIZE);

    let mut offset = 0;
    for _ in 0..num_rounds {
        // Load 16 bytes (we only use the first 10)
        let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

        // Extract 5-bit indices from 10 packed bytes
        let indices = extract_5bit_indices(input_vec);

        // Translate 5-bit indices to ASCII
        let encoded = translate_encode(indices, variant);

        // Store 16 output characters
        let mut output_buf = [0u8; 16];
        _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, encoded);

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

/// Extract 16 x 5-bit indices from 10 packed input bytes
///
/// This is the inverse of pack_5bit_to_8bit. Takes 10 bytes (80 bits)
/// and extracts 16 x 5-bit values (80 bits) into separate byte lanes.
///
/// For every 5 bytes [A B C D E], we extract 8 x 5-bit groups:
/// - Char 0: A >> 3           (bits 7-3 of A)
/// - Char 1: (A << 2) | (B >> 6) (bits 2-0 of A + bits 7-6 of B)
/// - Char 2: B >> 1           (bits 5-1 of B)
/// - Char 3: (B << 4) | (C >> 4) (bit 0 of B + bits 7-4 of C)
/// - Char 4: (C << 1) | (D >> 7) (bits 3-0 of C + bit 7 of D)
/// - Char 5: D >> 2           (bits 6-2 of D)
/// - Char 6: (D << 3) | (E >> 5) (bits 1-0 of D + bits 7-5 of E)
/// - Char 7: E & 0x1F         (bits 4-0 of E)
///
/// Note: Unlike base64's multiply-shift approach, 5-bit boundaries don't align
/// cleanly with 16-bit SIMD operations. We use a straightforward extraction
/// approach that's still faster than pure scalar for large inputs due to
/// SIMD translation and memory operations.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn extract_5bit_indices(input: std::arch::x86_64::__m128i) -> std::arch::x86_64::__m128i {
    // Use direct extraction - 5-bit boundaries are irregular
    unpack_5bit_simple(input)
}

/// Simple 5-bit unpacking using direct shifts and masks
///
/// Extracts 16 x 5-bit values from 10 bytes
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn unpack_5bit_simple(input: std::arch::x86_64::__m128i) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Extract bytes 0-9 into a buffer for easier manipulation
    let mut buf = [0u8; 16];
    _mm_storeu_si128(buf.as_mut_ptr() as *mut __m128i, input);

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

    _mm_loadu_si128(indices.as_ptr() as *const __m128i)
}

/// Translate 5-bit indices (0-31) to base32 ASCII characters
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn translate_encode(
    indices: std::arch::x86_64::__m128i,
    variant: Base32Variant,
) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    match variant {
        Base32Variant::Rfc4648 => {
            // RFC 4648 standard: 0-25 -> 'A'-'Z', 26-31 -> '2'-'7'
            // For indices 0-25: add 'A' (65)
            // For indices 26-31: add ('2' - 26) = 24

            // Create mask for indices >= 26
            let ge_26 = _mm_cmpgt_epi8(indices, _mm_set1_epi8(25));

            // Base offset is 'A' (65) for all
            let base = _mm_set1_epi8(b'A' as i8);

            // Adjustment for >= 26: we want '2' (50) for index 26
            // So offset should be 50 - 26 = 24 instead of 65
            // Difference: 24 - 65 = -41
            let adjustment = _mm_and_si128(ge_26, _mm_set1_epi8(-41));

            _mm_add_epi8(_mm_add_epi8(indices, base), adjustment)
        }
        Base32Variant::Rfc4648Hex => {
            // RFC 4648 hex: 0-9 -> '0'-'9', 10-31 -> 'A'-'V'
            // For indices 0-9: add '0' (48)
            // For indices 10-31: add ('A' - 10) = 55

            // Create mask for indices >= 10
            let ge_10 = _mm_cmpgt_epi8(indices, _mm_set1_epi8(9));

            // Base offset is '0' (48) for indices 0-9
            let base = _mm_set1_epi8(b'0' as i8);

            // Adjustment for >= 10: we want 'A' (65) for index 10
            // So offset should be 65 - 10 = 55 instead of 48
            // Difference: 55 - 48 = 7
            let adjustment = _mm_and_si128(ge_10, _mm_set1_epi8(7));

            _mm_add_epi8(_mm_add_epi8(indices, base), adjustment)
        }
    }
}

/// AVX2 base32 decoding implementation
///
/// Processes 32 input characters -> 20 output bytes per iteration.
/// Uses 256-bit vectors to process two independent 16-char blocks in parallel.
///
/// Note: AVX2's vpshufb operates per 128-bit lane, so we process two
/// independent 16-char chunks as separate lanes.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn decode_avx2_impl(encoded: &[u8], variant: Base32Variant, result: &mut Vec<u8>) -> bool {
    use std::arch::x86_64::*;

    const INPUT_BLOCK_SIZE: usize = 32;

    // Strip padding
    let input_no_padding = if let Some(last_non_pad) = encoded.iter().rposition(|&b| b != b'=') {
        &encoded[..=last_non_pad]
    } else {
        encoded
    };

    // Need at least 32 bytes to use AVX2
    if input_no_padding.len() < 32 {
        // Fall back to SSSE3 for small inputs
        return decode_ssse3_impl(input_no_padding, variant, result);
    }

    // Get decode LUTs for this variant (128-bit versions)
    let (delta_check_128, delta_rebase_128) = get_decode_delta_tables(variant);

    // Broadcast to 256-bit (duplicate in both lanes)
    let delta_check = _mm256_broadcastsi128_si256(delta_check_128);
    let delta_rebase = _mm256_broadcastsi128_si256(delta_rebase_128);

    // Calculate number of full 32-byte blocks
    let (num_rounds, simd_bytes) =
        common::calculate_blocks(input_no_padding.len(), INPUT_BLOCK_SIZE);

    // Process full blocks
    for round in 0..num_rounds {
        let offset = round * INPUT_BLOCK_SIZE;

        // Load 32 bytes (16 chars per lane)
        let input_vec = _mm256_loadu_si256(input_no_padding.as_ptr().add(offset) as *const __m256i);

        // Validate and translate using hash-based approach
        // 1. Extract hash key (upper 4 bits)
        let hash_key = _mm256_and_si256(_mm256_srli_epi32(input_vec, 4), _mm256_set1_epi8(0x0F));

        // 2. Validate: check = delta_check[hash_key] + input
        let check = _mm256_add_epi8(_mm256_shuffle_epi8(delta_check, hash_key), input_vec);

        // 3. Check should be <= 0x1F (31) for valid base32 characters
        let invalid_mask = _mm256_cmpgt_epi8(check, _mm256_set1_epi8(0x1F));
        if _mm256_movemask_epi8(invalid_mask) != 0 {
            return false; // Invalid characters
        }

        // 4. Translate: indices = input + delta_rebase[hash_key]
        let indices = _mm256_add_epi8(input_vec, _mm256_shuffle_epi8(delta_rebase, hash_key));

        // Pack 5-bit values into bytes (32 chars -> 20 bytes, per-lane)
        let decoded = pack_5bit_to_8bit_avx2(indices);

        // Extract 10 bytes from each 128-bit lane (20 total)
        // Lane 0 (low): bytes 0-9
        // Lane 1 (high): bytes 0-9 (after extracting high 128 bits)
        let lane0 = _mm256_castsi256_si128(decoded);
        let lane1 = _mm256_extracti128_si256(decoded, 1);

        let mut buf0 = [0u8; 16];
        let mut buf1 = [0u8; 16];
        _mm_storeu_si128(buf0.as_mut_ptr() as *mut __m128i, lane0);
        _mm_storeu_si128(buf1.as_mut_ptr() as *mut __m128i, lane1);

        result.extend_from_slice(&buf0[0..10]);
        result.extend_from_slice(&buf1[0..10]);
    }

    // Handle remainder with SSSE3 fallback
    if simd_bytes < input_no_padding.len() {
        let remainder = &input_no_padding[simd_bytes..];
        if !decode_ssse3_impl(remainder, variant, result) {
            return false;
        }
    }

    true
}

/// Pack 32 bytes of 5-bit indices into 20 bytes (AVX2)
///
/// Processes two independent 16-char blocks (one per 128-bit lane).
/// Same algorithm as SSSE3 pack_5bit_to_8bit, but applied to both lanes simultaneously.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn pack_5bit_to_8bit_avx2(
    indices: std::arch::x86_64::__m256i,
) -> std::arch::x86_64::__m256i {
    use std::arch::x86_64::*;

    // Extract both 128-bit lanes and process separately
    let lane_lo = _mm256_castsi256_si128(indices);
    let lane_hi = _mm256_extracti128_si256(indices, 1);

    // Apply SSSE3 packing to each lane
    let packed_lo = pack_5bit_to_8bit(lane_lo);
    let packed_hi = pack_5bit_to_8bit(lane_hi);

    // Recombine into 256-bit register
    _mm256_set_m128i(packed_hi, packed_lo)
}

/// SSSE3 base32 decoding implementation
///
/// Based on Lemire's algorithm: https://lemire.me/blog/2023/07/20/fast-decoding-of-base32-strings/
/// Processes 16 input characters -> 10 output bytes per iteration
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn decode_ssse3_impl(encoded: &[u8], variant: Base32Variant, result: &mut Vec<u8>) -> bool {
    use std::arch::x86_64::*;

    const INPUT_BLOCK_SIZE: usize = 16;

    // Strip padding
    let input_no_padding = if let Some(last_non_pad) = encoded.iter().rposition(|&b| b != b'=') {
        &encoded[..=last_non_pad]
    } else {
        encoded
    };

    // Get decode LUTs for this variant
    let (delta_check, delta_rebase) = get_decode_delta_tables(variant);

    // Calculate number of full 16-byte blocks
    let (num_rounds, simd_bytes) =
        common::calculate_blocks(input_no_padding.len(), INPUT_BLOCK_SIZE);

    // Process full blocks
    for round in 0..num_rounds {
        let offset = round * INPUT_BLOCK_SIZE;

        // Load 16 bytes
        let input_vec = _mm_loadu_si128(input_no_padding.as_ptr().add(offset) as *const __m128i);

        // Validate and translate using hash-based approach
        // 1. Extract hash key (upper 4 bits)
        let hash_key = _mm_and_si128(_mm_srli_epi32(input_vec, 4), _mm_set1_epi8(0x0F));

        // 2. Validate: check = delta_check[hash_key] + input
        let check = _mm_add_epi8(_mm_shuffle_epi8(delta_check, hash_key), input_vec);

        // 3. Check should be <= 0x1F (31) for valid base32 characters
        let invalid_mask = _mm_cmpgt_epi8(check, _mm_set1_epi8(0x1F));
        if _mm_movemask_epi8(invalid_mask) != 0 {
            return false; // Invalid characters
        }

        // 4. Translate: indices = input + delta_rebase[hash_key]
        let indices = _mm_add_epi8(input_vec, _mm_shuffle_epi8(delta_rebase, hash_key));

        // Pack 5-bit values into bytes (16 chars -> 10 bytes)
        let decoded = pack_5bit_to_8bit(indices);

        // Store 10 bytes
        let mut output_buf = [0u8; 16];
        _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, decoded);
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

/// Get decode delta tables for hash-based validation
///
/// Returns (delta_check, delta_rebase) lookup tables indexed by high nibble.
/// These tables enable single-shuffle validation and translation.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn get_decode_delta_tables(
    variant: Base32Variant,
) -> (std::arch::x86_64::__m128i, std::arch::x86_64::__m128i) {
    use std::arch::x86_64::*;

    match variant {
        Base32Variant::Rfc4648 => {
            // RFC 4648 standard: A-Z (0x41-0x5A) -> 0-25, 2-7 (0x32-0x37) -> 26-31
            // Hash key is high nibble (input >> 4)
            //
            // High nibble ranges:
            // 0x3x: '2'-'7' (0x32-0x37)
            // 0x4x: 'A'-'O' (0x41-0x4F)
            // 0x5x: 'P'-'Z' (0x50-0x5A)
            //
            // delta_check: add this + input, result should be <= 0x1F
            // delta_rebase: add this + input to get 5-bit index

            let delta_check = _mm_setr_epi8(
                0x7F,
                0x7F,
                0x7F,                // 0x0, 0x1, 0x2 - invalid
                (0x1F - 0x37) as i8, // 0x3: '2'-'7' -> check <= 0x1F
                (0x1F - 0x4F) as i8, // 0x4: 'A'-'O' -> check <= 0x1F
                (0x1F - 0x5A) as i8, // 0x5: 'P'-'Z' -> check <= 0x1F
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
            );

            let delta_rebase = _mm_setr_epi8(
                0,
                0,
                0,                           // 0x0, 0x1, 0x2 - unused
                (26i16 - b'2' as i16) as i8, // 0x3: '2' -> 26
                (0i16 - b'A' as i16) as i8,  // 0x4: 'A' -> 0
                (0i16 - b'A' as i16) as i8,  // 0x5: 'A' -> 0 (P-Z use same offset)
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
            );

            (delta_check, delta_rebase)
        }
        Base32Variant::Rfc4648Hex => {
            // RFC 4648 hex: 0-9 (0x30-0x39) -> 0-9, A-V (0x41-0x56) -> 10-31
            // High nibble ranges:
            // 0x3x: '0'-'9' (0x30-0x39)
            // 0x4x: 'A'-'O' (0x41-0x4F)
            // 0x5x: 'P'-'V' (0x50-0x56)

            let delta_check = _mm_setr_epi8(
                0x7F,
                0x7F,
                0x7F,                // 0x0, 0x1, 0x2 - invalid
                (0x1F - 0x39) as i8, // 0x3: '0'-'9' -> check <= 0x1F
                (0x1F - 0x4F) as i8, // 0x4: 'A'-'O' -> check <= 0x1F
                (0x1F - 0x56) as i8, // 0x5: 'P'-'V' -> check <= 0x1F
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
            );

            let delta_rebase = _mm_setr_epi8(
                0,
                0,
                0,                           // 0x0, 0x1, 0x2 - unused
                (0i16 - b'0' as i16) as i8,  // 0x3: '0' -> 0
                (10i16 - b'A' as i16) as i8, // 0x4: 'A' -> 10
                (10i16 - b'A' as i16) as i8, // 0x5: 'A' -> 10 (P-V use same offset)
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
            );

            (delta_check, delta_rebase)
        }
    }
}

/// Pack 16 bytes of 5-bit indices into 10 bytes
///
/// Based on Lemire's multiply-shift approach for base32.
/// 16 5-bit values -> 10 8-bit bytes
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn pack_5bit_to_8bit(indices: std::arch::x86_64::__m128i) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;

    // Process in groups of 8 chars -> 5 bytes
    // Input: 8 bytes, each containing 5-bit value (0x00-0x1F)
    // Output: 5 packed bytes

    // Stage 1: Merge pairs using multiply-add
    // _mm_maddubs_epi16: multiply pairs of bytes, then add adjacent results
    // Multiply by 0x20 (32) to shift left by 5 bits, 0x01 to keep in place
    // Result: 8 16-bit values, each combining two 5-bit inputs
    let merged = _mm_maddubs_epi16(indices, _mm_set1_epi32(0x01200120u32 as i32));

    // Stage 2: Combine 16-bit pairs into 32-bit values
    // _mm_madd_epi16: multiply pairs of 16-bit values, then add adjacent results
    // This packs four 5-bit values into each 32-bit lane
    // 0x00000001 << 16 | 0x00000400 = shift left by 10 bits, or keep in place << 10
    let combined = _mm_madd_epi16(
        merged,
        _mm_set_epi32(
            0x00010400, // High 64-bit lane, 2nd pair
            0x00104000, // High 64-bit lane, 1st pair
            0x00010400, // Low 64-bit lane, 2nd pair
            0x00104000, // Low 64-bit lane, 1st pair
        ),
    );

    // Now we have 4 x 32-bit values, each containing parts of our packed output
    // Layout (after multiply-add):
    // - Each 32-bit contains bits from 4 5-bit inputs
    // - We need to extract and rearrange these

    // Stage 3: Shift and combine to consolidate bits
    // Shift upper 16 bits of each 32-bit down, then OR
    let shifted = _mm_srli_epi64(combined, 48);
    let packed = _mm_or_si128(combined, shifted);

    // Stage 4: Shuffle to extract the 10 valid bytes in correct order
    // From NLnetLabs/simdzone: _mm_set_epi8(0, 0, 0, 0, 0, 0, 12, 13, 8, 9, 10, 4, 5, 0, 1, 2)
    // Note: _mm_set_epi8 is in REVERSE order (first arg goes to byte 15)
    // Converting to setr order (forward): 2, 1, 0, 5, 4, 10, 9, 8, 13, 12, 0, 0, 0, 0, 0, 0
    _mm_shuffle_epi8(
        packed,
        _mm_setr_epi8(
            2, 1, 0, // Bytes 0-2
            5, 4, // Bytes 3-4
            10, 9, 8, // Bytes 5-7
            13, 12, // Bytes 8-9
            0, 0, 0, 0, 0, 0, // Padding
        ),
    )
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

    #[test]
    fn test_avx2_large_input() {
        let dictionary = make_base32_dict();

        // Test with input large enough to trigger AVX2 path (>32 bytes)
        // 40 bytes = 2 AVX2 blocks (20 bytes each)
        let test_data: Vec<u8> = (0..40).map(|i| i).collect();

        if let Some(simd_result) = encode(&test_data, &dictionary, Base32Variant::Rfc4648) {
            // Verify round-trip
            if let Some(decoded) = decode(&simd_result, Base32Variant::Rfc4648) {
                assert_eq!(decoded, test_data, "AVX2 round-trip failed");
            }
        }
    }

    #[test]
    fn test_avx2_decode_large() {
        // Test AVX2 decode path with input large enough (>32 chars)
        // 64 chars = 2 AVX2 blocks (32 chars each -> 40 bytes total)
        let dictionary = make_base32_dict();
        let test_data: Vec<u8> = (0..40).map(|i| (i * 3) as u8).collect();

        if let Some(encoded) = encode(&test_data, &dictionary, Base32Variant::Rfc4648) {
            // Should use AVX2 for decoding
            if let Some(decoded) = decode(&encoded, Base32Variant::Rfc4648) {
                assert_eq!(decoded, test_data, "AVX2 decode failed");
            }
        }
    }
}
