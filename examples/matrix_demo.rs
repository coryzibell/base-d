use base_d::{decode, encode, Dictionary, DictionaryRegistry};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("WELCOME TO THE MATRIX");
    println!("================================\n");

    // Load Matrix base256 dictionary
    let config = DictionaryRegistry::load_default()?;
    let matrix_config = config.get_dictionary("base256_matrix").unwrap();

    let chars: Vec<char> = matrix_config.chars.chars().collect();
    let dictionary = Dictionary::new_with_mode(chars, matrix_config.mode.clone(), None)?;

    println!("Dictionary: base256_matrix");
    println!("Size: {} characters", dictionary.base());
    println!("Mode: {:?}", dictionary.mode());
    println!("Style: Katakana + Hiragana + Box Drawing + Geometric Shapes");
    println!();

    // The Matrix message
    let messages = vec![
        ("Wake up, Neo...", "Matrix Wake-Up Call"),
        ("Follow the white rabbit", "Follow the Rabbit"),
        ("There is no spoon", "The Spoon"),
        ("Free your mind", "Mind Liberation"),
    ];

    for (message, title) in messages {
        println!("{}", title);
        println!("Original: {}", message);

        let encoded = encode(message.as_bytes(), &dictionary);
        println!("Matrix:   {}", encoded);

        let decoded = decode(&encoded, &dictionary)?;
        let decoded_text = String::from_utf8_lossy(&decoded);
        println!("Decoded:  {}", decoded_text);
        println!();
    }

    // Demonstrate the special property
    println!("SPECIAL PROPERTY: Like Hexadecimal");
    println!("=======================================");
    println!("Base256 works identically in BOTH modes:");
    println!();

    let test_data = b"Matrix";

    // Test with chunked mode
    let chunked_alphabet = Dictionary::new_with_mode(
        matrix_config.chars.chars().collect(),
        base_d::EncodingMode::Chunked,
        None,
    )?;
    let chunked_encoded = encode(test_data, &chunked_alphabet);

    // Test with mathematical mode
    let math_alphabet = Dictionary::new_with_mode(
        matrix_config.chars.chars().collect(),
        base_d::EncodingMode::BaseConversion,
        None,
    )?;
    let math_encoded = encode(test_data, &math_alphabet);

    println!("Input:       '{}'", String::from_utf8_lossy(test_data));
    println!("Chunked:     {}", chunked_encoded);
    println!("Mathematical: {}", math_encoded);
    println!();

    if chunked_encoded == math_encoded {
        println!("IDENTICAL OUTPUT");
        println!("This works because:");
        println!("  - base256 = 2^8 (8 bits per character)");
        println!("  - 8 bits % 8 = 0 (perfect division)");
        println!("  - Same as hexadecimal, but Matrix-style");
    } else {
        println!("Outputs differ (unexpected)");
    }
    println!();

    // Information density comparison
    println!("EFFICIENCY COMPARISON");
    println!("========================");
    let long_message = b"The Matrix has you... Follow the white rabbit. Knock, knock, Neo.";

    let base64_config = config.get_dictionary("base64").unwrap();
    let base64_chars: Vec<char> = base64_config.chars.chars().collect();
    let base64_padding = base64_config
        .padding
        .as_ref()
        .and_then(|s| s.chars().next());
    let base64_alphabet =
        Dictionary::new_with_mode(base64_chars, base64_config.mode.clone(), base64_padding)?;

    let matrix_encoded = encode(long_message, &dictionary);
    let base64_encoded = encode(long_message, &base64_alphabet);

    println!("Message: {} bytes", long_message.len());
    println!();
    println!(
        "Matrix (base256):  {} chars",
        matrix_encoded.chars().count()
    );
    println!(
        "Base64:            {} chars",
        base64_encoded.chars().count()
    );
    println!();
    println!("Information density:");
    println!("  Matrix:  8 bits per character (1 char = 1 byte)");
    println!("  Base64:  6 bits per character");
    println!("  Hex:     4 bits per character");
    println!();
    println!("Matrix encoding is the MOST COMPACT:");
    println!("  - Same size as input (1:1 ratio)");
    println!("  - No padding needed");
    println!("  - Pure byte-to-character mapping");

    Ok(())
}
