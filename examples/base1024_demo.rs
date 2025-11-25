use base_d::{decode, encode, DictionariesConfig, Dictionary};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load base1024 dictionary - a 1024-character dictionary using CJK ideographs
    let config = DictionariesConfig::load_default()?;
    let base1024_config = config.get_dictionary("base1024").unwrap();

    let chars: Vec<char> = base1024_config.chars.chars().collect();
    let dictionary = Dictionary::new_with_mode(chars, base1024_config.mode.clone(), None)?;

    println!("Base1024 Dictionary Demo");
    println!("======================");
    println!("Dictionary size: {} characters", dictionary.base());
    println!("Encoding mode: {:?}", dictionary.mode());
    println!();

    // Demonstrate encoding efficiency
    let data = b"Hello, World! This is a test of the base1024 encoding system.";
    let encoded = encode(data, &dictionary);

    println!("Original data: {} bytes", data.len());
    println!("Original text: {}", String::from_utf8_lossy(data));
    println!();

    println!("Encoded ({} characters):", encoded.chars().count());
    println!("{}", encoded);
    println!();

    // Compare with base64
    let base64_config = config.get_dictionary("base64").unwrap();
    let base64_chars: Vec<char> = base64_config.chars.chars().collect();
    let base64_padding = base64_config
        .padding
        .as_ref()
        .and_then(|s| s.chars().next());
    let base64_alphabet =
        Dictionary::new_with_mode(base64_chars, base64_config.mode.clone(), base64_padding)?;

    let base64_encoded = encode(data, &base64_alphabet);

    println!("Base64 comparison:");
    println!("  Base1024: {} characters", encoded.chars().count());
    println!("  Base64:   {} characters", base64_encoded.chars().count());
    println!(
        "  Savings:  {} characters ({:.1}% smaller)",
        base64_encoded.chars().count() - encoded.chars().count(),
        100.0 * (1.0 - encoded.chars().count() as f64 / base64_encoded.chars().count() as f64)
    );
    println!();

    // Decode
    let decoded = decode(&encoded, &dictionary)?;
    assert_eq!(decoded, data);

    println!("Decoded successfully!");
    println!("Decoded text: {}", String::from_utf8_lossy(&decoded));
    println!();

    // Information density
    println!("Information density:");
    println!("  Base64:   6 bits per character (2^6 = 64)");
    println!("  Base1024: 10 bits per character (2^10 = 1024)");
    println!("  Base1024 is {:.1}x more compact", 10.0 / 6.0);

    Ok(())
}
