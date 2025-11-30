use crate::core::dictionary::Dictionary;
use num_integer::lcm;

pub use super::errors::DecodeError;

#[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "aarch64")))]
use crate::simd;

pub fn encode_chunked(data: &[u8], dictionary: &Dictionary) -> String {
    // Try unified SIMD auto-selection
    #[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        if let Some(result) = simd::encode_with_simd(data, dictionary) {
            return result;
        }
    }

    // Fall back to scalar implementation
    encode_chunked_scalar(data, dictionary)
}

fn encode_chunked_scalar(data: &[u8], dictionary: &Dictionary) -> String {
    let base = dictionary.base();
    let bits_per_char = (base as f64).log2() as usize;

    if bits_per_char == 0 {
        return String::new();
    }

    // Pre-calculate output size for better memory allocation
    let output_bits = data.len() * 8;
    let output_chars = output_bits.div_ceil(bits_per_char);
    let capacity = if dictionary.padding().is_some() {
        output_chars.div_ceil(4) * 4
    } else {
        output_chars
    };
    let mut result = String::with_capacity(capacity);

    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0usize;

    // Process in chunks for better CPU cache utilization
    const PROCESS_CHUNK: usize = 64;
    let chunks = data.chunks_exact(PROCESS_CHUNK);
    let remainder = chunks.remainder();

    // Process main chunks
    for chunk in chunks {
        for &byte in chunk {
            bit_buffer = (bit_buffer << 8) | (byte as u32);
            bits_in_buffer += 8;

            while bits_in_buffer >= bits_per_char {
                bits_in_buffer -= bits_per_char;
                let index = ((bit_buffer >> bits_in_buffer) & ((1 << bits_per_char) - 1)) as usize;
                result.push(dictionary.encode_digit(index).unwrap());
            }
        }
    }

    // Process remainder
    for &byte in remainder {
        bit_buffer = (bit_buffer << 8) | (byte as u32);
        bits_in_buffer += 8;

        while bits_in_buffer >= bits_per_char {
            bits_in_buffer -= bits_per_char;
            let index = ((bit_buffer >> bits_in_buffer) & ((1 << bits_per_char) - 1)) as usize;
            result.push(dictionary.encode_digit(index).unwrap());
        }
    }

    // Handle remaining bits
    if bits_in_buffer > 0 {
        let index = ((bit_buffer << (bits_per_char - bits_in_buffer)) & ((1 << bits_per_char) - 1))
            as usize;
        result.push(dictionary.encode_digit(index).unwrap());
    }

    // Add padding if specified
    if let Some(pad_char) = dictionary.padding() {
        // Calculate padding group size based on LCM(bits_per_char, 8) / bits_per_char
        // Base64: LCM(6,8)=24, group=24/6=4
        // Base32: LCM(5,8)=40, group=40/5=8
        // Base16: LCM(4,8)=8, group=8/4=2
        let lcm = lcm(bits_per_char, 8);
        let group_size = lcm / bits_per_char;
        let padded_chars = result.len().div_ceil(group_size) * group_size;

        while result.len() < padded_chars {
            result.push(pad_char);
        }
    }

    result
}

pub fn decode_chunked(encoded: &str, dictionary: &Dictionary) -> Result<Vec<u8>, DecodeError> {
    if encoded.is_empty() {
        return Err(DecodeError::EmptyInput);
    }

    // Try unified SIMD auto-selection
    #[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        if let Some(result) = simd::decode_with_simd(encoded, dictionary) {
            return Ok(result);
        }
    }

    // Fall back to scalar implementation
    decode_chunked_scalar(encoded, dictionary)
}

fn decode_chunked_scalar(encoded: &str, dictionary: &Dictionary) -> Result<Vec<u8>, DecodeError> {
    let base = dictionary.base();
    let bits_per_char = (base as f64).log2() as usize;
    let padding = dictionary.padding();

    // Pre-allocate output buffer with estimated size
    let estimated_output = (encoded.len() * bits_per_char) / 8;
    let mut result = Vec::with_capacity(estimated_output);

    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0usize;

    // Collect chars once for better cache performance
    let chars: Vec<char> = encoded.chars().collect();

    // Build valid character string for error messages
    let valid_chars = if base <= 64 {
        (0..base)
            .filter_map(|i| dictionary.encode_digit(i))
            .collect::<String>()
    } else {
        format!("{} characters in dictionary", base)
    };

    // Track character position for error reporting
    let mut char_position = 0;

    // Process in chunks for better CPU cache utilization
    const CHUNK_SIZE: usize = 64;
    let chunks = chars.chunks_exact(CHUNK_SIZE);
    let remainder = chunks.remainder();

    // Process main chunks
    for chunk in chunks {
        for &c in chunk {
            // Handle padding
            if Some(c) == padding {
                return Ok(result);
            }

            let digit = dictionary.decode_char(c).ok_or_else(|| {
                DecodeError::invalid_character(c, char_position, encoded, &valid_chars)
            })?;

            bit_buffer = (bit_buffer << bits_per_char) | (digit as u32);
            bits_in_buffer += bits_per_char;

            while bits_in_buffer >= 8 {
                bits_in_buffer -= 8;
                let byte = ((bit_buffer >> bits_in_buffer) & 0xFF) as u8;
                result.push(byte);
            }

            char_position += 1;
        }
    }

    // Process remainder
    for &c in remainder {
        // Handle padding
        if Some(c) == padding {
            break;
        }

        let digit = dictionary.decode_char(c).ok_or_else(|| {
            DecodeError::invalid_character(c, char_position, encoded, &valid_chars)
        })?;

        bit_buffer = (bit_buffer << bits_per_char) | (digit as u32);
        bits_in_buffer += bits_per_char;

        while bits_in_buffer >= 8 {
            bits_in_buffer -= 8;
            let byte = ((bit_buffer >> bits_in_buffer) & 0xFF) as u8;
            result.push(byte);
        }

        char_position += 1;
    }

    Ok(result)
}
