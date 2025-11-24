use crate::core::dictionary::Dictionary;
use num_traits::Zero;
use num_integer::Integer;

/// Errors that can occur during decoding.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The input contains a character not in the dictionary
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

pub fn encode(data: &[u8], dictionary: &Dictionary) -> String {
    if data.is_empty() {
        return String::new();
    }
    
    // Count leading zeros for efficient handling
    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();
    
    // If all zeros, return early
    if leading_zeros == data.len() {
        return dictionary.encode_digit(0).unwrap().to_string().repeat(data.len());
    }
    
    let base = dictionary.base();
    let mut num = num_bigint::BigUint::from_bytes_be(&data[leading_zeros..]);
    
    // Pre-allocate result vector with estimated capacity
    let max_digits = ((data.len() - leading_zeros) * 8 * 1000) / (base as f64).log2() as usize / 1000 + 1;
    let mut result = Vec::with_capacity(max_digits + leading_zeros);
    
    let base_big = num_bigint::BigUint::from(base);
    
    while !num.is_zero() {
        let (quotient, remainder) = num.div_rem(&base_big);
        let digit = remainder.to_u64_digits();
        let digit_val = if digit.is_empty() { 0 } else { digit[0] as usize };
        result.push(dictionary.encode_digit(digit_val).unwrap());
        num = quotient;
    }
    
    // Add leading zeros
    for _ in 0..leading_zeros {
        result.push(dictionary.encode_digit(0).unwrap());
    }
    
    result.reverse();
    result.into_iter().collect()
}

pub fn decode(encoded: &str, dictionary: &Dictionary) -> Result<Vec<u8>, DecodeError> {
    if encoded.is_empty() {
        return Err(DecodeError::EmptyInput);
    }
    
    let base = dictionary.base();
    let mut num = num_bigint::BigUint::from(0u8);
    let base_big = num_bigint::BigUint::from(base);
    
    // Collect chars once for better cache performance
    let chars: Vec<char> = encoded.chars().collect();
    let mut leading_zeros = 0;
    
    // Process in chunks for better performance
    for &c in &chars {
        let digit = dictionary.decode_char(c)
            .ok_or(DecodeError::InvalidCharacter(c))?;
        
        if num.is_zero() && digit == 0 {
            leading_zeros += 1;
        } else {
            num *= &base_big;
            num += num_bigint::BigUint::from(digit);
        }
    }
    
    // Handle all-zero case
    if num.is_zero() && leading_zeros > 0 {
        return Ok(vec![0u8; leading_zeros]);
    }
    
    let bytes = num.to_bytes_be();
    
    // Construct result with pre-allocated capacity
    let mut result = Vec::with_capacity(leading_zeros + bytes.len());
    result.resize(leading_zeros, 0u8);
    result.extend_from_slice(&bytes);
    
    Ok(result)
}
