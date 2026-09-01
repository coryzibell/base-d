use super::errors::{DecodeError, EncodeError};
use crate::core::dictionary::Dictionary;

/// Convert a codepoint to a char, returning an EncodeError if it is invalid.
///
/// This covers the case where a byte maps to a surrogate or otherwise invalid
/// Unicode codepoint. In practice this should never fire because the Dictionary
/// builder rejects unsafe `start_codepoint` values, but returning a Result
/// ensures the library never panics on invalid input.
fn safe_char_from_codepoint(codepoint: u32, start: u32, byte: u8) -> Result<char, EncodeError> {
    std::char::from_u32(codepoint).ok_or(EncodeError::InvalidCodepoint {
        codepoint,
        start_codepoint: start,
        byte,
    })
}

/// Encode data using byte range mode (direct byte-to-character mapping).
/// Each byte maps to `start_codepoint + byte_value`.
///
/// # Errors
///
/// Returns `EncodeError::InvalidCodepoint` if any byte maps to an invalid
/// Unicode codepoint (e.g., surrogates U+D800-U+DFFF). This indicates the
/// dictionary was constructed with an unsafe `start_codepoint`.
/// With the current builder validation via `is_safe_byte_range()`, this error
/// should never occur for properly constructed dictionaries.
pub fn encode_byte_range(data: &[u8], dictionary: &Dictionary) -> Result<String, EncodeError> {
    let start = dictionary
        .start_codepoint()
        .expect("ByteRange mode requires start_codepoint");

    // Pre-allocate with exact capacity for better performance
    let mut result = String::with_capacity(data.len() * 4); // Max 4 bytes per UTF-8 char

    // Process in chunks for better CPU cache utilization
    const CHUNK_SIZE: usize = 64;
    let (chunks, remainder) = data.as_chunks::<CHUNK_SIZE>();

    for chunk in chunks {
        for &byte in chunk {
            let codepoint = start + byte as u32;
            let c = safe_char_from_codepoint(codepoint, start, byte)?;
            result.push(c);
        }
    }

    // Process remainder
    for &byte in remainder {
        let codepoint = start + byte as u32;
        let c = safe_char_from_codepoint(codepoint, start, byte)?;
        result.push(c);
    }

    Ok(result)
}

/// Decode data using byte range mode
pub fn decode_byte_range(encoded: &str, dictionary: &Dictionary) -> Result<Vec<u8>, DecodeError> {
    let start = dictionary
        .start_codepoint()
        .expect("ByteRange mode requires start_codepoint");

    let char_count = encoded.chars().count();
    let mut result = Vec::with_capacity(char_count);

    // Build valid range string for error messages
    let valid_chars = format!("U+{:04X} to U+{:04X}", start, start + 255);

    // Track position for error reporting
    let mut char_position = 0;

    // Process in chunks for better cache utilization
    const CHUNK_SIZE: usize = 64;
    let chars: Vec<char> = encoded.chars().collect();
    let (chunks, remainder) = chars.as_chunks::<CHUNK_SIZE>();

    for chunk in chunks {
        for &c in chunk {
            let codepoint = c as u32;
            if codepoint >= start && codepoint < start + 256 {
                result.push((codepoint - start) as u8);
            } else {
                return Err(DecodeError::invalid_character(
                    c,
                    char_position,
                    encoded,
                    &valid_chars,
                ));
            }
            char_position += 1;
        }
    }

    // Process remainder
    for &c in remainder {
        let codepoint = c as u32;
        if codepoint >= start && codepoint < start + 256 {
            result.push((codepoint - start) as u8);
        } else {
            return Err(DecodeError::invalid_character(
                c,
                char_position,
                encoded,
                &valid_chars,
            ));
        }
        char_position += 1;
    }

    Ok(result)
}

#[cfg(test)]
#[allow(deprecated)]
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
        let encoded = encode_byte_range(data, &dictionary).unwrap();
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
        let encoded = encode_byte_range(&data, &dictionary).unwrap();
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
        let encoded = encode_byte_range(data, &dictionary).unwrap();
        let decoded = decode_byte_range(&encoded, &dictionary).unwrap();

        assert_eq!(data, &decoded[..]);
    }
}
