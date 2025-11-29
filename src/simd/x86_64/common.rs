//! Common utilities and scaffolding for SIMD implementations
//!
//! This module contains shared functionality that doesn't hurt performance:
//! - CPU feature detection helpers
//! - Block size calculations
//! - Common test utilities
//!
//! The actual hot path (reshuffle, translate) remains specialized per bit-width.

use crate::core::dictionary::Dictionary;

/// Check if SSSE3 is available (required for pshufb)
#[inline(always)]
#[allow(dead_code)] // Reserved for future use
pub fn has_ssse3() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("ssse3")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Calculate number of full blocks and remainder offset
///
/// Returns (num_full_blocks, simd_processed_bytes)
#[inline(always)]
pub fn calculate_blocks(data_len: usize, block_size: usize) -> (usize, usize) {
    let num_blocks = data_len / block_size;
    let simd_bytes = num_blocks * block_size;
    (num_blocks, simd_bytes)
}

/// Scalar encoding for power-of-two bases (chunked bit-width encoding)
///
/// This is the fallback for remainder bytes in SIMD encoding.
/// Works for any bit-width that divides evenly into powers of 2.
#[inline]
pub fn encode_scalar_chunked(data: &[u8], dictionary: &Dictionary, result: &mut String) {
    // Get bits per character from dictionary base
    let base = dictionary.base();
    let bits_per_char = (base as f64).log2() as usize;

    if bits_per_char == 0 || bits_per_char > 8 {
        // Not a power-of-two base, shouldn't happen
        return;
    }

    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0usize;
    let mask = (1u32 << bits_per_char) - 1;

    for &byte in data {
        bit_buffer = (bit_buffer << 8) | (byte as u32);
        bits_in_buffer += 8;

        while bits_in_buffer >= bits_per_char {
            bits_in_buffer -= bits_per_char;
            let index = ((bit_buffer >> bits_in_buffer) & mask) as usize;
            if let Some(ch) = dictionary.encode_digit(index) {
                result.push(ch);
            }
        }
    }

    // Handle final bits (padding with zeros on the right)
    if bits_in_buffer > 0 {
        let index = ((bit_buffer << (bits_per_char - bits_in_buffer)) & mask) as usize;
        if let Some(ch) = dictionary.encode_digit(index) {
            result.push(ch);
        }
    }

    // Add padding if specified (e.g., base64 '=')
    if let Some(pad_char) = dictionary.padding() {
        // Calculate expected output length
        let input_bits = data.len() * 8;
        let output_chars = input_bits.div_ceil(bits_per_char);

        // For base64, output should be multiple of 4
        if base == 64 {
            while !result.len().is_multiple_of(4) {
                result.push(pad_char);
            }
        } else {
            // For other bases, pad to expected length
            while result.len() < output_chars {
                result.push(pad_char);
            }
        }
    }
}

/// Scalar decoding for power-of-two bases (chunked bit-width decoding)
///
/// This is the fallback for remainder bytes in SIMD decoding.
pub fn decode_scalar_chunked<F>(
    data: &[u8],
    char_to_index: &mut F,
    result: &mut Vec<u8>,
    bits_per_char: usize,
) -> bool
where
    F: FnMut(u8) -> Option<u8> + ?Sized,
{
    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0usize;

    for &byte in data {
        let index = match char_to_index(byte) {
            Some(i) => i as u32,
            None => return false,
        };

        bit_buffer = (bit_buffer << bits_per_char) | index;
        bits_in_buffer += bits_per_char;

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

    #[test]
    fn test_calculate_blocks() {
        // Test exact multiple
        assert_eq!(calculate_blocks(48, 16), (3, 48));

        // Test with remainder
        assert_eq!(calculate_blocks(50, 16), (3, 48));

        // Test smaller than block
        assert_eq!(calculate_blocks(10, 16), (0, 0));

        // Test zero
        assert_eq!(calculate_blocks(0, 16), (0, 0));
    }

    #[test]
    fn test_has_ssse3() {
        // Just verify it doesn't panic
        let _ = has_ssse3();
    }
}
