//! Specific test for emoji_animals dictionary nul byte bug

use base_d::prelude::*;

#[test]
fn test_emoji_animals_gzip_nul() {
    let msg = "Session wrap: wake ritual, bloom tagging, Honeypot check-in, 109 blooms";

    // Compress with gzip
    let compressed = compress(msg.as_bytes(), CompressionAlgorithm::Gzip, 6).unwrap();

    println!("\n=== Testing emoji_animals with Gzip Compressed Data ===\n");
    println!("Original: {}", msg);
    println!("Compressed: {} bytes", compressed.len());

    // Show compressed bytes
    let hex: Vec<String> = compressed.iter().map(|b| format!("{:02x}", b)).collect();
    println!("Compressed hex: {}", hex.join(" "));

    let nul_count = compressed.iter().filter(|&&b| b == 0).count();
    println!("Nul bytes in compressed: {}", nul_count);

    // Load registry and get emoji_animals dictionary
    let registry = DictionaryRegistry::load_default().unwrap();
    let dict = registry.dictionary("emoji_animals").unwrap();

    println!("\nDictionary: emoji_animals");
    println!("Mode: {:?}", dict.mode());
    println!("Start codepoint: {:?}", dict.start_codepoint());

    // Encode
    let encoded = encode(&compressed, &dict);

    println!("\nEncoded length: {} chars", encoded.chars().count());

    // Check for nul
    if encoded.contains('\0') {
        println!("❌ ENCODED STRING CONTAINS NUL!");

        // Find all nul positions
        let nul_positions: Vec<usize> = encoded
            .chars()
            .enumerate()
            .filter(|(_, c)| *c == '\0')
            .map(|(i, _)| i)
            .collect();

        println!("Nul positions: {:?}", nul_positions);

        // Show preview with control chars escaped
        let preview: String = encoded
            .chars()
            .take(50)
            .map(|c| {
                if c.is_control() {
                    format!("\\u{:04X}", c as u32)
                } else {
                    c.to_string()
                }
            })
            .collect();

        println!("Preview (first 50 chars): {}", preview);

        // Try to decode and see what we get
        let decoded = decode(&encoded, &dict).unwrap();
        println!("\nDecoded length: {} bytes", decoded.len());
        println!("Original compressed length: {} bytes", compressed.len());

        if decoded == compressed {
            println!("✓ Round-trip preserves data (but contains nul in encoded form)");
        } else {
            println!("❌ Round-trip corrupted!");
            let bytes_lost = compressed.len() as i64 - decoded.len() as i64;
            println!("Bytes difference: {}", bytes_lost);
        }
    } else {
        println!("✓ No nul in encoded string");
    }
}
