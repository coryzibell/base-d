use crate::alphabet::Alphabet;

pub use crate::encoding::DecodeError;

pub fn encode_chunked(data: &[u8], alphabet: &Alphabet) -> String {
    let base = alphabet.base();
    let bits_per_char = (base as f64).log2() as usize;
    
    if bits_per_char == 0 {
        return String::new();
    }
    
    let mut result = String::new();
    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0usize;
    
    for &byte in data {
        bit_buffer = (bit_buffer << 8) | (byte as u32);
        bits_in_buffer += 8;
        
        while bits_in_buffer >= bits_per_char {
            bits_in_buffer -= bits_per_char;
            let index = ((bit_buffer >> bits_in_buffer) & ((1 << bits_per_char) - 1)) as usize;
            result.push(alphabet.encode_digit(index).unwrap());
        }
    }
    
    // Handle remaining bits
    if bits_in_buffer > 0 {
        let index = ((bit_buffer << (bits_per_char - bits_in_buffer)) & ((1 << bits_per_char) - 1)) as usize;
        result.push(alphabet.encode_digit(index).unwrap());
    }
    
    // Add padding if specified
    if let Some(pad_char) = alphabet.padding() {
        let input_bits = data.len() * 8;
        let output_chars = (input_bits + bits_per_char - 1) / bits_per_char;
        let padded_chars = ((output_chars + 3) / 4) * 4; // Round up to multiple of 4
        
        while result.len() < padded_chars {
            result.push(pad_char);
        }
    }
    
    result
}

pub fn decode_chunked(encoded: &str, alphabet: &Alphabet) -> Result<Vec<u8>, DecodeError> {
    if encoded.is_empty() {
        return Err(DecodeError::EmptyInput);
    }
    
    let base = alphabet.base();
    let bits_per_char = (base as f64).log2() as usize;
    let padding = alphabet.padding();
    
    let mut result = Vec::new();
    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0usize;
    
    for c in encoded.chars() {
        // Handle padding
        if Some(c) == padding {
            break;
        }
        
        let digit = alphabet.decode_char(c)
            .ok_or(DecodeError::InvalidCharacter(c))?;
        
        bit_buffer = (bit_buffer << bits_per_char) | (digit as u32);
        bits_in_buffer += bits_per_char;
        
        while bits_in_buffer >= 8 {
            bits_in_buffer -= 8;
            let byte = ((bit_buffer >> bits_in_buffer) & 0xFF) as u8;
            result.push(byte);
        }
    }
    
    Ok(result)
}
