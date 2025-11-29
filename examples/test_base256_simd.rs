use base_d::{Dictionary, DictionaryRegistry, decode, encode};

fn main() {
    // Load dictionary
    let config = DictionaryRegistry::load_default().unwrap();
    let dict_config = config.get_dictionary("base256_matrix").unwrap();
    let chars: Vec<char> = dict_config.chars.chars().collect();
    let dict = Dictionary::builder()
        .chars(chars)
        .mode(dict_config.effective_mode())
        .build()
        .unwrap();

    println!("Testing base256 SIMD implementation...\n");

    // Test 1: Small data (< 16 bytes, should use scalar fallback)
    let small = b"Hello!";
    let encoded_small = encode(small, &dict);
    let decoded_small = decode(&encoded_small, &dict).unwrap();
    println!(
        "✓ Small test (6 bytes): {}",
        if decoded_small == small {
            "PASS"
        } else {
            "FAIL"
        }
    );
    assert_eq!(encoded_small.chars().count(), small.len());
    assert_eq!(decoded_small, small);

    // Test 2: Exactly 16 bytes (one SIMD block)
    let exact16: Vec<u8> = (0..16).collect();
    let encoded_16 = encode(&exact16, &dict);
    let decoded_16 = decode(&encoded_16, &dict).unwrap();
    println!(
        "✓ SIMD boundary (16 bytes): {}",
        if decoded_16 == exact16 {
            "PASS"
        } else {
            "FAIL"
        }
    );
    assert_eq!(encoded_16.chars().count(), 16);
    assert_eq!(decoded_16, exact16);

    // Test 3: 17 bytes (one SIMD block + 1 remainder)
    let plus_one: Vec<u8> = (0..17).collect();
    let encoded_17 = encode(&plus_one, &dict);
    let decoded_17 = decode(&encoded_17, &dict).unwrap();
    println!(
        "✓ SIMD + remainder (17 bytes): {}",
        if decoded_17 == plus_one {
            "PASS"
        } else {
            "FAIL"
        }
    );
    assert_eq!(encoded_17.chars().count(), 17);
    assert_eq!(decoded_17, plus_one);

    // Test 4: All 256 byte values
    let all_bytes: Vec<u8> = (0..=255).collect();
    let encoded_all = encode(&all_bytes, &dict);
    let decoded_all = decode(&encoded_all, &dict).unwrap();
    println!(
        "✓ All bytes (256): {}",
        if decoded_all == all_bytes {
            "PASS"
        } else {
            "FAIL"
        }
    );
    assert_eq!(encoded_all.chars().count(), 256);
    assert_eq!(decoded_all, all_bytes);

    // Test 5: Large data (multiple SIMD blocks)
    let large: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    let encoded_large = encode(&large, &dict);
    let decoded_large = decode(&encoded_large, &dict).unwrap();
    println!(
        "✓ Large data (1KB): {}",
        if decoded_large == large {
            "PASS"
        } else {
            "FAIL"
        }
    );
    assert_eq!(encoded_large.chars().count(), 1024);
    assert_eq!(decoded_large, large);

    // Verify 1:1 property
    println!("\n✓ Base256 maintains 1:1 byte-to-char ratio (perfect encoding)");

    println!("\n✓✓✓ All base256 SIMD tests passed! ✓✓✓");
}
