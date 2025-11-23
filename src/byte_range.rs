use crate::alphabet::Alphabet;
use crate::encoding::DecodeError;

/// Encode data using byte range mode (direct byte-to-character mapping)
/// Each byte maps to start_codepoint + byte_value
pub fn encode_byte_range(data: &[u8], alphabet: &Alphabet) -> String {
    let start = alphabet.start_codepoint()
        .expect("ByteRange mode requires start_codepoint");
    
    data.iter()
        .filter_map(|&byte| std::char::from_u32(start + byte as u32))
        .collect()
}

/// Decode data using byte range mode
pub fn decode_byte_range(encoded: &str, alphabet: &Alphabet) -> Result<Vec<u8>, DecodeError> {
    let start = alphabet.start_codepoint()
        .expect("ByteRange mode requires start_codepoint");
    
    let mut result = Vec::with_capacity(encoded.chars().count());
    
    for c in encoded.chars() {
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
    use crate::config::EncodingMode;
    
    #[test]
    fn test_byte_range_encode_decode() {
        let alphabet = Alphabet::new_with_mode_and_range(
            Vec::new(),
            EncodingMode::ByteRange,
            None,
            Some(0x1F3F7), // Base100 emoji start
        ).unwrap();
        
        let data = b"Hello, World!";
        let encoded = encode_byte_range(data, &alphabet);
        let decoded = decode_byte_range(&encoded, &alphabet).unwrap();
        
        assert_eq!(data, &decoded[..]);
    }
    
    #[test]
    fn test_byte_range_all_bytes() {
        let alphabet = Alphabet::new_with_mode_and_range(
            Vec::new(),
            EncodingMode::ByteRange,
            None,
            Some(0x1F3F7),
        ).unwrap();
        
        // Test all 256 possible byte values
        let data: Vec<u8> = (0..=255).collect();
        let encoded = encode_byte_range(&data, &alphabet);
        let decoded = decode_byte_range(&encoded, &alphabet).unwrap();
        
        assert_eq!(data, decoded);
    }
    
    #[test]
    fn test_byte_range_empty() {
        let alphabet = Alphabet::new_with_mode_and_range(
            Vec::new(),
            EncodingMode::ByteRange,
            None,
            Some(0x1F3F7),
        ).unwrap();
        
        let data = b"";
        let encoded = encode_byte_range(data, &alphabet);
        let decoded = decode_byte_range(&encoded, &alphabet).unwrap();
        
        assert_eq!(data, &decoded[..]);
    }
}
