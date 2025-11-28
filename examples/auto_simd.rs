//! Demonstration of automatic SIMD selection
//!
//! This example shows how the library automatically uses SIMD acceleration
//! for compatible dictionaries without requiring manual configuration.

use base_d::{encode, Dictionary};

fn main() {
    let data = b"Hello, World! This is a test of automatic SIMD selection.";

    println!("=== Automatic SIMD Selection Demo ===\n");

    // 1. Standard base64 - uses specialized SIMD
    let base64_chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let dict = Dictionary::new(base64_chars.chars().collect()).unwrap();
    let encoded = encode(data, &dict);
    println!("Standard base64 (specialized SIMD):");
    println!("  {}\n", encoded);

    // 2. Standard hex - uses specialized SIMD
    let hex_chars = "0123456789abcdef";
    let dict = Dictionary::new(hex_chars.chars().collect()).unwrap();
    let encoded = encode(data, &dict);
    println!("Standard hex (specialized SIMD):");
    println!("  {}\n", encoded);

    // 3. Custom sequential base16 - uses GenericSimdCodec
    let custom_hex: Vec<char> = (0x21..0x31).map(|cp| char::from_u32(cp).unwrap()).collect();
    let dict = Dictionary::new(custom_hex).unwrap();
    let encoded = encode(data, &dict);
    println!("Custom base16 starting at '!' (GenericSimdCodec):");
    println!("  {}\n", encoded);

    // 4. Custom sequential base64 - uses GenericSimdCodec
    let custom_b64: Vec<char> = (0x100..0x140)
        .map(|cp| char::from_u32(cp).unwrap())
        .collect();
    let dict = Dictionary::new(custom_b64).unwrap();
    let encoded = encode(data, &dict);
    println!("Custom base64 at U+0100 (GenericSimdCodec):");
    println!("  {}\n", encoded);

    // 5. Arbitrary dictionary - falls back to scalar
    let arbitrary = "ZYXWVUTSRQPONMLKJIHGFEDCBAzyxwvutsrqponmlkjihgfedcba9876543210+/";
    let dict = Dictionary::new(arbitrary.chars().collect()).unwrap();
    let encoded = encode(data, &dict);
    println!("Arbitrary shuffled base64 (scalar fallback):");
    println!("  {}\n", encoded);

    println!("=== Selection Order ===");
    println!("1. Known base64 variants (standard/url) → specialized base64 SIMD");
    println!("2. Known hex variants → specialized base16 SIMD");
    println!("3. Base256 ByteRange → specialized base256 SIMD");
    println!("4. Sequential power-of-2 dictionary → GenericSimdCodec");
    println!("5. None → scalar fallback");
}
