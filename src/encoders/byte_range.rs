use crate::core::dictionary::Dictionary;
use crate::encoders::encoding::DecodeError;

/// Encode data using byte range mode (direct byte-to-character mapping)
/// Each byte maps to start_codepoint + byte_value
pub fn encode_byte_range(data: &[u8], dictionary: &Dictionary) -> String {
    let start = dictionary
        .start_codepoint()
        .expect("ByteRange mode requires start_codepoint");

    // Pre-allocate with exact capacity for better performance
    let mut result = String::with_capacity(data.len() * 4); // Max 4 bytes per UTF-8 char

    // Process in chunks for better CPU cache utilization
    const CHUNK_SIZE: usize = 64;
    let chunks = data.chunks_exact(CHUNK_SIZE);
    let remainder = chunks.remainder();

    for chunk in chunks {
        for &byte in chunk {
            if let Some(c) = std::char::from_u32(start + byte as u32) {
                result.push(c);
            }
        }
    }

    // Process remainder
    for &byte in remainder {
        if let Some(c) = std::char::from_u32(start + byte as u32) {
            result.push(c);
        }
    }

    result
}

/// Decode data using byte range mode
pub fn decode_byte_range(encoded: &str, dictionary: &Dictionary) -> Result<Vec<u8>, DecodeError> {
    let start = dictionary
        .start_codepoint()
        .expect("ByteRange mode requires start_codepoint");

    let char_count = encoded.chars().count();
    let mut result = Vec::with_capacity(char_count);

    // Process in chunks for better cache utilization
    const CHUNK_SIZE: usize = 64;
    let chars: Vec<char> = encoded.chars().collect();
    let chunks = chars.chunks_exact(CHUNK_SIZE);
    let remainder = chunks.remainder();

    for chunk in chunks {
        for &c in chunk {
            let codepoint = c as u32;
            if codepoint >= start && codepoint < start + 256 {
                result.push((codepoint - start) as u8);
            } else {
                return Err(DecodeError::InvalidCharacter(c));
            }
        }
    }

    // Process remainder
    for &c in remainder {
        let codepoint = c as u32;
        if codepoint >= start && codepoint < start + 256 {
            result.push((codepoint - start) as u8);
        } else {
            return Err(DecodeError::InvalidCharacter(c));
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::EncodingMode;

    #[test]
    fn test_byte_range_encode_decode() {
        let dictionary = Dictionary::new_with_mode_and_range(
            Vec::new(),
            EncodingMode::ByteRange,
            None,
            Some(0x1F3F7), // Base100 emoji start
        )
        .unwrap();

        let data = b"Hello, World!";
        let encoded = encode_byte_range(data, &dictionary);
        let decoded = decode_byte_range(&encoded, &dictionary).unwrap();

        assert_eq!(data, &decoded[..]);
    }

    #[test]
    fn test_byte_range_all_bytes() {
        let dictionary = Dictionary::new_with_mode_and_range(
            Vec::new(),
            EncodingMode::ByteRange,
            None,
            Some(0x1F3F7),
        )
        .unwrap();

        // Test all 256 possible byte values
        let data: Vec<u8> = (0..=255).collect();
        let encoded = encode_byte_range(&data, &dictionary);
        let decoded = decode_byte_range(&encoded, &dictionary).unwrap();

        assert_eq!(data, decoded);
    }

    #[test]
    fn test_byte_range_empty() {
        let dictionary = Dictionary::new_with_mode_and_range(
            Vec::new(),
            EncodingMode::ByteRange,
            None,
            Some(0x1F3F7),
        )
        .unwrap();

        let data = b"";
        let encoded = encode_byte_range(data, &dictionary);
        let decoded = decode_byte_range(&encoded, &dictionary).unwrap();

        assert_eq!(data, &decoded[..]);
    }
}
