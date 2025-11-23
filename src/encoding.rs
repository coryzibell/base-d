use crate::alphabet::Alphabet;
use num_traits::Zero;

/// Errors that can occur during decoding.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The input contains a character not in the alphabet
    InvalidCharacter(char),
    /// The input string is empty
    EmptyInput,
    /// The padding is malformed or incorrect
    InvalidPadding,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::InvalidCharacter(c) => write!(f, "Invalid character in input: {}", c),
            DecodeError::EmptyInput => write!(f, "Cannot decode empty input"),
            DecodeError::InvalidPadding => write!(f, "Invalid padding"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub fn encode(data: &[u8], alphabet: &Alphabet) -> String {
    if data.is_empty() {
        return String::new();
    }
    
    // Count leading zeros
    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();
    
    // If all zeros, just return the zero character repeated
    if leading_zeros == data.len() {
        return alphabet.encode_digit(0).unwrap().to_string().repeat(data.len());
    }
    
    let base = alphabet.base();
    let mut num = num_bigint::BigUint::from_bytes_be(&data[leading_zeros..]);
    let mut result = Vec::new();
    let base_big = num_bigint::BigUint::from(base);
    
    while !num.is_zero() {
        let remainder = &num % &base_big;
        let digit = remainder.to_u64_digits();
        let digit_val = if digit.is_empty() { 0 } else { digit[0] as usize };
        result.push(alphabet.encode_digit(digit_val).unwrap());
        num /= &base_big;
    }
    
    // Add leading zeros
    for _ in 0..leading_zeros {
        result.push(alphabet.encode_digit(0).unwrap());
    }
    
    result.reverse();
    result.into_iter().collect()
}

pub fn decode(encoded: &str, alphabet: &Alphabet) -> Result<Vec<u8>, DecodeError> {
    if encoded.is_empty() {
        return Err(DecodeError::EmptyInput);
    }
    
    let base = alphabet.base();
    let mut num = num_bigint::BigUint::from(0u8);
    let base_big = num_bigint::BigUint::from(base);
    
    let chars: Vec<char> = encoded.chars().collect();
    let mut leading_zeros = 0;
    
    for &c in &chars {
        let digit = alphabet.decode_char(c)
            .ok_or(DecodeError::InvalidCharacter(c))?;
        
        if num.is_zero() && digit == 0 {
            leading_zeros += 1;
        } else {
            num = num * &base_big + num_bigint::BigUint::from(digit);
        }
    }
    
    let bytes = num.to_bytes_be();
    
    if num.is_zero() && leading_zeros > 0 {
        return Ok(vec![0u8; leading_zeros]);
    }
    
    // Add leading zero bytes
    let mut result = vec![0u8; leading_zeros];
    result.extend_from_slice(&bytes);
    
    Ok(result)
}
