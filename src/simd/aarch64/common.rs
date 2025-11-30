//! Common utilities for aarch64 SIMD implementations
//!
//! This module provides shared helper functions used across different
//! bit-width SIMD encoders/decoders on aarch64.

use crate::core::dictionary::Dictionary;

/// Calculate the number of complete blocks and total bytes that can be processed
///
/// # Arguments
/// * `len` - Total length of input data
/// * `block_size` - Size of each SIMD block to process
///
/// # Returns
/// A tuple of (num_blocks, bytes_processed) where:
/// - `num_blocks` is the number of complete blocks that fit in the input
/// - `bytes_processed` is the total number of bytes covered by those blocks
#[inline]
pub fn calculate_blocks(len: usize, block_size: usize) -> (usize, usize) {
    let num_blocks = len / block_size;
    let bytes_processed = num_blocks * block_size;
    (num_blocks, bytes_processed)
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
        // For base64, output should be multiple of 4
        if base == 64 {
            while !result.len().is_multiple_of(4) {
                result.push(pad_char);
            }
        }
    }
}

/// Scalar decoding for power-of-two bases (chunked bit-width decoding)
///
/// This is the fallback for remainder bytes in SIMD decoding.
/// Works for any bit-width that divides evenly into powers of 2.
///
/// # Arguments
/// * `data` - The encoded data to decode
/// * `char_to_index` - Function to convert a character to its index value
/// * `result` - Output buffer for decoded bytes
/// * `bits_per_char` - Number of bits per encoded character (6 for base64, 5 for base32, 4 for base16)
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
