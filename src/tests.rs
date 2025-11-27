use crate::{decode, encode, DictionaryRegistry, Dictionary, EncodingMode};

fn get_dictionary(name: &str) -> Dictionary {
    let config = DictionaryRegistry::load_default().unwrap();
    let alphabet_config = config.get_dictionary(name).unwrap();

    match alphabet_config.mode {
        EncodingMode::ByteRange => {
            let start = alphabet_config.start_codepoint.unwrap();
            Dictionary::new_with_mode_and_range(
                Vec::new(),
                alphabet_config.mode.clone(),
                None,
                Some(start),
            )
            .unwrap()
        }
        _ => {
            let chars: Vec<char> = alphabet_config.chars.chars().collect();
            let padding = alphabet_config
                .padding
                .as_ref()
                .and_then(|s| s.chars().next());
            Dictionary::new_with_mode(chars, alphabet_config.mode.clone(), padding).unwrap()
        }
    }
}

#[test]
fn test_encode_decode_empty() {
    let dictionary = get_dictionary("cards");
    let data = b"";
    let encoded = encode(data, &dictionary);
    assert_eq!(encoded, "");
}

#[test]
fn test_encode_decode_zero() {
    let dictionary = get_dictionary("cards");
    let data = &[0u8];
    let encoded = encode(data, &dictionary);
    assert_eq!(encoded.chars().count(), 1);
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_encode_decode_simple() {
    let dictionary = get_dictionary("cards");
    let data = b"Hello";
    let encoded = encode(data, &dictionary);
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_encode_decode_hello_world() {
    let dictionary = get_dictionary("cards");
    let data = b"Hello, World!";
    let encoded = encode(data, &dictionary);
    println!("Encoded: {}", encoded);
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_encode_decode_binary() {
    let dictionary = get_dictionary("cards");
    let data = &[0u8, 1, 2, 3, 255, 254, 253];
    let encoded = encode(data, &dictionary);
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_encode_decode_leading_zeros() {
    let dictionary = get_dictionary("cards");
    let data = &[0u8, 0, 0, 1, 2, 3];
    let encoded = encode(data, &dictionary);
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_decode_invalid_character() {
    let dictionary = get_dictionary("cards");
    let result = decode("ABC", &dictionary);
    assert!(result.is_err());
}

#[test]
fn test_alphabet_base() {
    let dictionary = get_dictionary("cards");
    assert_eq!(dictionary.base(), 52);
}

#[test]
fn test_base64_chunked_mode() {
    let dictionary = get_dictionary("base64");
    assert_eq!(dictionary.mode(), &EncodingMode::Chunked);

    // Test standard base64 encoding
    let data = b"Hello, World!";
    let encoded = encode(data, &dictionary);
    println!("base64 encoded: {}", encoded);

    // Should match standard base64
    let expected = "SGVsbG8sIFdvcmxkIQ==";
    assert_eq!(encoded, expected);

    // Test decoding
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base64_math_mode() {
    let dictionary = get_dictionary("base64_math");
    assert_eq!(dictionary.mode(), &EncodingMode::BaseConversion);

    // This should use mathematical base conversion
    let data = b"Hello, World!";
    let encoded = encode(data, &dictionary);
    println!("base64_math encoded: {}", encoded);

    // Should NOT match standard base64
    let standard_base64 = "SGVsbG8sIFdvcmxkIQ==";
    assert_ne!(encoded, standard_base64);

    // But should still round-trip
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base100_byte_range_mode() {
    let dictionary = get_dictionary("base100");
    assert_eq!(dictionary.mode(), &EncodingMode::ByteRange);
    assert_eq!(dictionary.base(), 256);

    // Test simple encoding
    let data = b"Hello, World!";
    let encoded = encode(data, &dictionary);
    println!("base100 encoded: {}", encoded);

    // Each byte should map to exactly one emoji
    assert_eq!(encoded.chars().count(), data.len());

    // Verify specific codepoints for first few characters
    // 'H' = 72, should map to 127991 + 72 = 128063 (U+1F43F)
    let first_char = encoded.chars().next().unwrap();
    assert_eq!(first_char as u32, 127991 + 72);

    // Test decoding
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base100_all_bytes() {
    let dictionary = get_dictionary("base100");

    // Test all 256 possible byte values
    let data: Vec<u8> = (0..=255).collect();
    let encoded = encode(&data, &dictionary);

    // Should encode to 256 emojis
    assert_eq!(encoded.chars().count(), 256);

    // Should round-trip correctly
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base100_empty() {
    let dictionary = get_dictionary("base100");

    let data = b"";
    let encoded = encode(data, &dictionary);
    assert_eq!(encoded, "");

    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base100_binary_data() {
    let dictionary = get_dictionary("base100");

    let data = &[0u8, 1, 2, 3, 255, 254, 253, 128, 127];
    let encoded = encode(data, &dictionary);
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base1024_large_alphabet() {
    // Test that we can load and use a 1024-character dictionary
    let dictionary = get_dictionary("base1024");

    // Verify base size
    assert_eq!(dictionary.base(), 1024);

    // Test encoding/decoding various data sizes
    let test_data = vec![
        b"A".to_vec(),
        b"Hello".to_vec(),
        b"Hello, World!".to_vec(),
        (0u8..=255).collect::<Vec<u8>>(), // All bytes
    ];

    for data in test_data {
        let encoded = encode(&data, &dictionary);
        let decoded = decode(&encoded, &dictionary).unwrap();
        assert_eq!(decoded, data, "Failed for data of length {}", data.len());

        // Verify that encoding with larger base produces shorter output
        // For mathematical mode, larger base = more compact representation
        // Each 1024-base digit represents ~10 bits (log2(1024) = 10)
        let bits_in = data.len() * 8;
        let max_chars = (bits_in + 9) / 10; // ceiling division
        assert!(
            encoded.chars().count() <= max_chars + 1,
            "Encoding too long: {} chars for {} bytes (expected <= {})",
            encoded.chars().count(),
            data.len(),
            max_chars + 1
        );
    }
}

#[test]
fn test_base1024_uses_hashmap() {
    // Base1024 uses non-ASCII characters, so it should use HashMap not lookup table
    let dictionary = get_dictionary("base1024");

    // Test that decoding works correctly (verifies HashMap fallback)
    let data = b"Testing large dictionary HashMap fallback";
    let encoded = encode(data, &dictionary);
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base1024_efficiency() {
    let dictionary = get_dictionary("base1024");

    // Compare with base64 for same data
    let base64 = get_dictionary("base64");
    let data = b"The quick brown fox jumps over the lazy dog";

    let encoded_1024 = encode(data, &dictionary);
    let encoded_64 = encode(data, &base64);

    // Base1024 should produce fewer characters than base64
    // base1024: ~10 bits per char, base64: 6 bits per char
    assert!(
        encoded_1024.chars().count() < encoded_64.chars().count(),
        "Base1024 ({} chars) should be shorter than base64 ({} chars)",
        encoded_1024.chars().count(),
        encoded_64.chars().count()
    );
}

#[test]
fn test_base256_matrix_like_hex() {
    // Test that base256_matrix works identically in both modes (like hexadecimal)
    let alphabet_chunked = get_dictionary("base256_matrix");

    // Verify it's a 256-character dictionary
    assert_eq!(alphabet_chunked.base(), 256);

    // Create mathematical mode version
    let config = DictionaryRegistry::load_default().unwrap();
    let matrix_config = config.get_dictionary("base256_matrix").unwrap();
    let chars: Vec<char> = matrix_config.chars.chars().collect();
    let alphabet_math =
        Dictionary::new_with_mode(chars, EncodingMode::BaseConversion, None).unwrap();

    // Test various data sizes
    let test_data = vec![
        b"A".to_vec(),
        b"Hi".to_vec(),
        b"Matrix".to_vec(),
        b"The Matrix has you...".to_vec(),
        (0u8..=255).collect::<Vec<u8>>(), // All bytes
    ];

    for data in test_data {
        let chunked_encoded = encode(&data, &alphabet_chunked);
        let math_encoded = encode(&data, &alphabet_math);

        // Both modes should produce IDENTICAL output (like hexadecimal)
        assert_eq!(
            chunked_encoded,
            math_encoded,
            "Modes should produce identical output for {} bytes (like hex!)",
            data.len()
        );

        // Verify round-trip
        let decoded = decode(&chunked_encoded, &alphabet_chunked).unwrap();
        assert_eq!(decoded, data);

        // Verify 1:1 mapping (256 = 2^8 = 1 byte per char)
        assert_eq!(
            chunked_encoded.chars().count(),
            data.len(),
            "Base256 should have 1:1 char-to-byte ratio"
        );
    }
}

#[test]
fn test_base256_matrix_perfect_encoding() {
    let dictionary = get_dictionary("base256_matrix");

    // Test the special property: 8 bits % log2(256) = 8 % 8 = 0
    // This means no expansion, perfect 1:1 mapping
    let data = b"Follow the white rabbit";
    let encoded = encode(data, &dictionary);

    // Should be exactly the same length
    assert_eq!(encoded.chars().count(), data.len());

    // Decode should work perfectly
    let decoded = decode(&encoded, &dictionary).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_base256_matrix_all_bytes() {
    let dictionary = get_dictionary("base256_matrix");

    // Test that all 256 possible byte values can be encoded/decoded
    let all_bytes: Vec<u8> = (0..=255).collect();
    let encoded = encode(&all_bytes, &dictionary);
    let decoded = decode(&encoded, &dictionary).unwrap();

    assert_eq!(decoded, all_bytes);
    assert_eq!(encoded.chars().count(), 256); // 1:1 ratio
}
