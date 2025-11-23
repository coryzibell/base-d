use crate::{encode, decode, AlphabetsConfig, Alphabet, EncodingMode};

fn get_alphabet(name: &str) -> Alphabet {
    let config = AlphabetsConfig::load_default().unwrap();
    let alphabet_config = config.get_alphabet(name).unwrap();
    
    match alphabet_config.mode {
        EncodingMode::ByteRange => {
            let start = alphabet_config.start_codepoint.unwrap();
            Alphabet::new_with_mode_and_range(Vec::new(), alphabet_config.mode.clone(), None, Some(start)).unwrap()
        }
        _ => {
            let chars: Vec<char> = alphabet_config.chars.chars().collect();
            let padding = alphabet_config.padding.as_ref().and_then(|s| s.chars().next());
            Alphabet::new_with_mode(chars, alphabet_config.mode.clone(), padding).unwrap()
        }
    }
}

#[test]
fn test_encode_decode_empty() {
    let alphabet = get_alphabet("cards");
    let data = b"";
    let encoded = encode(data, &alphabet);
    assert_eq!(encoded, "");
}

#[test]
fn test_encode_decode_zero() {
    let alphabet = get_alphabet("cards");
    let data = &[0u8];
    let encoded = encode(data, &alphabet);
    assert_eq!(encoded.chars().count(), 1);
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_encode_decode_simple() {
    let alphabet = get_alphabet("cards");
    let data = b"Hello";
    let encoded = encode(data, &alphabet);
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_encode_decode_hello_world() {
    let alphabet = get_alphabet("cards");
    let data = b"Hello, World!";
    let encoded = encode(data, &alphabet);
    println!("Encoded: {}", encoded);
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_encode_decode_binary() {
    let alphabet = get_alphabet("cards");
    let data = &[0u8, 1, 2, 3, 255, 254, 253];
    let encoded = encode(data, &alphabet);
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_encode_decode_leading_zeros() {
    let alphabet = get_alphabet("cards");
    let data = &[0u8, 0, 0, 1, 2, 3];
    let encoded = encode(data, &alphabet);
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_decode_invalid_character() {
    let alphabet = get_alphabet("cards");
    let result = decode("ABC", &alphabet);
    assert!(result.is_err());
}

#[test]
fn test_alphabet_base() {
    let alphabet = get_alphabet("cards");
    assert_eq!(alphabet.base(), 52);
}

#[test]
fn test_base64_chunked_mode() {
    let alphabet = get_alphabet("base64");
    assert_eq!(alphabet.mode(), &EncodingMode::Chunked);
    
    // Test standard base64 encoding
    let data = b"Hello, World!";
    let encoded = encode(data, &alphabet);
    println!("base64 encoded: {}", encoded);
    
    // Should match standard base64
    let expected = "SGVsbG8sIFdvcmxkIQ==";
    assert_eq!(encoded, expected);
    
    // Test decoding
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base64_math_mode() {
    let alphabet = get_alphabet("base64_math");
    assert_eq!(alphabet.mode(), &EncodingMode::BaseConversion);
    
    // This should use mathematical base conversion
    let data = b"Hello, World!";
    let encoded = encode(data, &alphabet);
    println!("base64_math encoded: {}", encoded);
    
    // Should NOT match standard base64
    let standard_base64 = "SGVsbG8sIFdvcmxkIQ==";
    assert_ne!(encoded, standard_base64);
    
    // But should still round-trip
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base100_byte_range_mode() {
    let alphabet = get_alphabet("base100");
    assert_eq!(alphabet.mode(), &EncodingMode::ByteRange);
    assert_eq!(alphabet.base(), 256);
    
    // Test simple encoding
    let data = b"Hello, World!";
    let encoded = encode(data, &alphabet);
    println!("base100 encoded: {}", encoded);
    
    // Each byte should map to exactly one emoji
    assert_eq!(encoded.chars().count(), data.len());
    
    // Verify specific codepoints for first few characters
    // 'H' = 72, should map to 127991 + 72 = 128063 (U+1F43F)
    let first_char = encoded.chars().next().unwrap();
    assert_eq!(first_char as u32, 127991 + 72);
    
    // Test decoding
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base100_all_bytes() {
    let alphabet = get_alphabet("base100");
    
    // Test all 256 possible byte values
    let data: Vec<u8> = (0..=255).collect();
    let encoded = encode(&data, &alphabet);
    
    // Should encode to 256 emojis
    assert_eq!(encoded.chars().count(), 256);
    
    // Should round-trip correctly
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base100_empty() {
    let alphabet = get_alphabet("base100");
    
    let data = b"";
    let encoded = encode(data, &alphabet);
    assert_eq!(encoded, "");
    
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base100_binary_data() {
    let alphabet = get_alphabet("base100");
    
    let data = &[0u8, 1, 2, 3, 255, 254, 253, 128, 127];
    let encoded = encode(data, &alphabet);
    let decoded = decode(&encoded, &alphabet).unwrap();
    assert_eq!(decoded, data);
}
