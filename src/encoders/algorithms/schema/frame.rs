use super::display96;
use super::types::SchemaError;
use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::Zero;

/// Egyptian hieroglyph quotation marks - parser-inert frame delimiters
/// These characters are chosen because:
/// - They're visually distinctive
/// - They're unlikely to appear in parsers/syntax highlighters
/// - They clearly denote "special encoded content"
pub const FRAME_START: char = '𓍹'; // U+13379 EGYPTIAN HIEROGLYPH V011A
pub const FRAME_END: char = '𓍺'; // U+1337A EGYPTIAN HIEROGLYPH V011B

/// Encode binary data with display96 and wrap in frame delimiters
///
/// # Algorithm
/// 1. Convert binary bytes to base-96 using display96 alphabet
/// 2. Prepend FRAME_START delimiter
/// 3. Append FRAME_END delimiter
///
/// # Example
/// ```ignore
/// let binary = vec![0x01, 0x02, 0x03];
/// let framed = encode_framed(&binary);
/// // Returns: "𓍹{base96_encoded_content}𓍺"
/// ```
pub fn encode_framed(binary: &[u8]) -> String {
    let encoded = encode_base96(binary);
    format!("{}{}{}", FRAME_START, encoded, FRAME_END)
}

/// Remove frame delimiters and decode display96 back to binary
///
/// # Errors
/// - Returns `InvalidFrame` if delimiters are missing or malformed
/// - Returns `InvalidCharacter` if non-alphabet chars are found
///
/// # Example
/// ```ignore
/// let framed = "𓍹{base96_content}𓍺";
/// let binary = decode_framed(framed)?;
/// ```
pub fn decode_framed(encoded: &str) -> Result<Vec<u8>, SchemaError> {
    // Validate frame delimiters
    if !encoded.starts_with(FRAME_START) {
        return Err(SchemaError::InvalidFrame(format!(
            "Missing start delimiter '{}' (U+{:04X})",
            FRAME_START, FRAME_START as u32
        )));
    }

    if !encoded.ends_with(FRAME_END) {
        return Err(SchemaError::InvalidFrame(format!(
            "Missing end delimiter '{}' (U+{:04X})",
            FRAME_END, FRAME_END as u32
        )));
    }

    // Strip delimiters
    let start_len = FRAME_START.len_utf8();
    let end_len = FRAME_END.len_utf8();
    let content = &encoded[start_len..encoded.len() - end_len];

    // Decode base96
    decode_base96(content)
}

/// Encode bytes to display96 string using base-96 radix conversion
///
/// Uses BigUint for arbitrary precision, similar to radix.rs approach.
fn encode_base96(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }

    // Count leading zeros for efficient handling
    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();

    // If all zeros, return early
    if leading_zeros == data.len() {
        return display96::char_at(0)
            .unwrap()
            .to_string()
            .repeat(data.len());
    }

    let base = 96u32;
    let mut num = BigUint::from_bytes_be(&data[leading_zeros..]);

    // Pre-allocate result vector
    let max_digits =
        ((data.len() - leading_zeros) * 8 * 1000) / (base as f64).log2() as usize / 1000 + 1;
    let mut result = Vec::with_capacity(max_digits + leading_zeros);

    let base_big = BigUint::from(base);

    while !num.is_zero() {
        let (quotient, remainder) = num.div_rem(&base_big);
        let digit = remainder.to_u64_digits();
        let digit_val = if digit.is_empty() {
            0
        } else {
            digit[0] as usize
        };
        result.push(display96::char_at(digit_val).unwrap());
        num = quotient;
    }

    // Add leading zeros
    for _ in 0..leading_zeros {
        result.push(display96::char_at(0).unwrap());
    }

    result.reverse();
    result.into_iter().collect()
}

/// Decode display96 string to bytes using base-96 radix conversion
fn decode_base96(encoded: &str) -> Result<Vec<u8>, SchemaError> {
    if encoded.is_empty() {
        return Ok(Vec::new());
    }

    let base = 96u32;
    let mut num = BigUint::from(0u8);
    let base_big = BigUint::from(base);

    let chars: Vec<char> = encoded.chars().collect();
    let mut leading_zeros = 0;

    for (pos, &c) in chars.iter().enumerate() {
        let digit = display96::index_of(c).ok_or_else(|| {
            SchemaError::InvalidCharacter(format!(
                "Invalid character '{}' (U+{:04X}) at position {}. Must be in display96 alphabet",
                c, c as u32, pos
            ))
        })?;

        if num.is_zero() && digit == 0 {
            leading_zeros += 1;
        } else {
            num *= &base_big;
            num += BigUint::from(digit);
        }
    }

    // Handle all-zero case
    if num.is_zero() && leading_zeros > 0 {
        return Ok(vec![0u8; leading_zeros]);
    }

    let bytes = num.to_bytes_be();

    // Construct result with leading zeros
    let mut result = Vec::with_capacity(leading_zeros + bytes.len());
    result.resize(leading_zeros, 0u8);
    result.extend_from_slice(&bytes);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_delimiters() {
        assert_eq!(FRAME_START as u32, 0x13379);
        assert_eq!(FRAME_END as u32, 0x1337A);
    }

    #[test]
    fn test_encode_decode_empty() {
        let binary = vec![];
        let framed = encode_framed(&binary);
        assert_eq!(framed, format!("{}{}", FRAME_START, FRAME_END));

        let decoded = decode_framed(&framed).unwrap();
        assert_eq!(decoded, binary);
    }

    #[test]
    fn test_encode_decode_single_byte() {
        let binary = vec![42];
        let framed = encode_framed(&binary);
        assert!(framed.starts_with(FRAME_START));
        assert!(framed.ends_with(FRAME_END));

        let decoded = decode_framed(&framed).unwrap();
        assert_eq!(decoded, binary);
    }

    #[test]
    fn test_encode_decode_multiple_bytes() {
        let binary = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        let framed = encode_framed(&binary);

        let decoded = decode_framed(&framed).unwrap();
        assert_eq!(decoded, binary);
    }

    #[test]
    fn test_encode_decode_zeros() {
        let binary = vec![0x00, 0x00, 0x00];
        let framed = encode_framed(&binary);

        let decoded = decode_framed(&framed).unwrap();
        assert_eq!(decoded, binary);
    }

    #[test]
    fn test_encode_decode_leading_zeros() {
        let binary = vec![0x00, 0x00, 0x42, 0xFF];
        let framed = encode_framed(&binary);

        let decoded = decode_framed(&framed).unwrap();
        assert_eq!(decoded, binary);
    }

    #[test]
    fn test_encode_decode_large_values() {
        let binary = vec![0xFF; 32]; // 32 bytes of 0xFF
        let framed = encode_framed(&binary);

        let decoded = decode_framed(&framed).unwrap();
        assert_eq!(decoded, binary);
    }

    #[test]
    fn test_decode_missing_start_delimiter() {
        let encoded = format!("test{}", FRAME_END);
        let result = decode_framed(&encoded);
        assert!(matches!(result, Err(SchemaError::InvalidFrame(_))));
    }

    #[test]
    fn test_decode_missing_end_delimiter() {
        let encoded = format!("{}test", FRAME_START);
        let result = decode_framed(&encoded);
        assert!(matches!(result, Err(SchemaError::InvalidFrame(_))));
    }

    #[test]
    fn test_decode_invalid_character() {
        let encoded = format!("{}ABC{}", FRAME_START, FRAME_END);
        let result = decode_framed(&encoded);
        assert!(matches!(result, Err(SchemaError::InvalidCharacter(_))));
    }

    #[test]
    fn test_base96_roundtrip() {
        let test_cases = vec![
            vec![0x00],
            vec![0x01],
            vec![0xFF],
            vec![0x01, 0x02, 0x03],
            vec![0x00, 0x42],
            vec![0x42, 0x00],
            (0..=255).collect::<Vec<u8>>(),
        ];

        for binary in test_cases {
            let encoded = encode_base96(&binary);
            let decoded = decode_base96(&encoded).unwrap();
            assert_eq!(decoded, binary, "Failed for input: {:02X?}", binary);
        }
    }

    #[test]
    fn test_visual_output() {
        let test_data = b"Hello, world!";
        let framed = encode_framed(test_data);

        println!("Input: {:02X?}", test_data);
        println!("Framed output: {}", framed);
        println!("Length: {} chars", framed.chars().count());

        let decoded = decode_framed(&framed).unwrap();
        assert_eq!(decoded, test_data);
    }
}
