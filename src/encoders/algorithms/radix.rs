use crate::core::dictionary::Dictionary;
use num_integer::Integer;
use num_traits::Zero;

pub use super::errors::DecodeError;

pub fn encode(data: &[u8], dictionary: &Dictionary) -> String {
    if data.is_empty() {
        return String::new();
    }

    // Count leading zeros for efficient handling
    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();

    // If all zeros, return early
    if leading_zeros == data.len() {
        return dictionary
            .encode_digit(0)
            .unwrap()
            .to_string()
            .repeat(data.len());
    }

    let base = dictionary.base();
    let mut num = num_bigint::BigUint::from_bytes_be(&data[leading_zeros..]);

    // Pre-allocate result vector with estimated capacity
    let max_digits =
        ((data.len() - leading_zeros) * 8 * 1000) / (base as f64).log2() as usize / 1000 + 1;
    let mut result = Vec::with_capacity(max_digits + leading_zeros);

    let base_big = num_bigint::BigUint::from(base);

    while !num.is_zero() {
        let (quotient, remainder) = num.div_rem(&base_big);
        let digit = remainder.to_u64_digits();
        let digit_val = if digit.is_empty() {
            0
        } else {
            digit[0] as usize
        };
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

    // Build valid character string for error messages (truncate if too long)
    let valid_chars = if let Some(start) = dictionary.start_codepoint() {
        format!("U+{:04X} to U+{:04X}", start, start + 255)
    } else {
        let base_val = dictionary.base();
        // Show first few and last few chars for large dictionaries
        if base_val <= 64 {
            (0..base_val)
                .filter_map(|i| dictionary.encode_digit(i))
                .collect::<String>()
        } else {
            format!("{} characters in dictionary", base_val)
        }
    };

    // Process in chunks for better performance - track position for error reporting
    let mut byte_position = 0;
    for &c in &chars {
        let digit = dictionary.decode_char(c).ok_or_else(|| {
            DecodeError::invalid_character(c, byte_position, encoded, &valid_chars)
        })?;

        if num.is_zero() && digit == 0 {
            leading_zeros += 1;
        } else {
            num *= &base_big;
            num += num_bigint::BigUint::from(digit);
        }

        // Track byte position (handles multi-byte UTF-8)
        byte_position += c.len_utf8();
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
