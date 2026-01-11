//! Fuzz tests for nul byte bug in compression + encoding pipeline
//!
//! Issue: https://github.com/coryzibell/base-d/issues/125
//!
//! Problem: Certain compression outputs contain nul bytes (\0) which cause
//! "nul byte found in provided data" errors when passed to git -m.
//!
//! Root cause: ByteRange encoding with start_codepoint values that map
//! compressed bytes to invalid Unicode codepoints, causing silent byte drops.

use base_d::prelude::*;

/// Test if compressed data contains nul bytes
#[test]
fn test_compression_nul_bytes() {
    let test_messages = vec![
        "add whirlpool command - three spiral methods with random selection",
        "Session wrap: wake ritual, bloom tagging, Honeypot check-in, 109 blooms",
        "Session wrap: activate script includeCoAuthoredBy fix",
        "Add iri command - bismuth staircase crystals with rainbow colors",
        "add whirlpool command - three spiral methods with random selection",
    ];

    println!("\n=== Testing Compression for Nul Bytes ===\n");

    for msg in test_messages {
        println!("Message: \"{}\"", msg);
        println!("Length: {} bytes\n", msg.len());

        for algo in [
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Snappy,
            CompressionAlgorithm::Brotli,
            CompressionAlgorithm::Gzip,
            CompressionAlgorithm::Lzma,
            CompressionAlgorithm::Zstd,
        ] {
            let level = algo.default_level();
            let compressed = compress(msg.as_bytes(), algo, level).unwrap();

            let nul_count = compressed.iter().filter(|&&b| b == 0).count();
            let has_nuls = nul_count > 0;

            println!(
                "  {:?}: {} bytes, {} nul bytes {}",
                algo,
                compressed.len(),
                nul_count,
                if has_nuls { "⚠️" } else { "✓" }
            );

            if has_nuls {
                // Show byte distribution
                let mut byte_counts = [0usize; 256];
                for &b in &compressed {
                    byte_counts[b as usize] += 1;
                }

                // Show problematic byte ranges
                let low_bytes = (0..32).filter(|&i| byte_counts[i] > 0).count();
                let surrogates_low = (0xD8..=0xDB).filter(|&i| byte_counts[i] > 0).count();
                let surrogates_high = (0xDC..=0xDF).filter(|&i| byte_counts[i] > 0).count();

                println!(
                    "    Problematic bytes: {} in [0-31], {} in [D8-DB], {} in [DC-DF]",
                    low_bytes, surrogates_low, surrogates_high
                );
            }
        }
        println!();
    }
}

/// Test encoding compressed data through different dictionaries
#[test]
fn test_encoding_compressed_with_nuls() {
    let msg = "add whirlpool command - three spiral methods with random selection";

    println!("\n=== Testing Encoding with Nul-Containing Compressed Data ===\n");

    // Compress with gzip (known to produce nuls)
    let algo = CompressionAlgorithm::Gzip;
    let level = algo.default_level();
    let compressed = compress(msg.as_bytes(), algo, level).unwrap();

    let nul_count = compressed.iter().filter(|&&b| b == 0).count();
    println!(
        "Compressed with {:?}: {} bytes, {} nuls",
        algo,
        compressed.len(),
        nul_count
    );

    if nul_count == 0 {
        println!(
            "⚠️ This message doesn't produce nuls with {:?} - test may not be representative",
            algo
        );
        return;
    }

    println!("✓ Using compression output with {} nul bytes\n", nul_count);

    // Load dictionary registry
    let registry = DictionaryRegistry::load_default().unwrap();

    // Test encoding through different dictionary types
    let test_dictionaries = vec!["base16", "base32", "base64", "cards", "dna", "base100"];

    for dict_name in test_dictionaries {
        if let Ok(dict) = registry.dictionary(dict_name) {
            let encoded = encode(&compressed, &dict);

            println!("\n  Dictionary: {}", dict_name);
            println!("    Mode: {:?}", dict.mode());
            println!("    Start codepoint: {:?}", dict.start_codepoint());
            println!("    Encoded length: {} chars", encoded.chars().count());
            println!(
                "    Expected length: {} chars (if all bytes encoded)",
                compressed.len()
            );

            // Check if encoding dropped bytes
            let decoded = decode(&encoded, &dict).unwrap();
            let bytes_lost = compressed.len() - decoded.len();

            if bytes_lost > 0 {
                println!(
                    "    ❌ BYTES DROPPED: {} bytes lost during round-trip!",
                    bytes_lost
                );
                println!("    This is THE BUG!");
            } else if compressed == decoded {
                println!("    ✓ Perfect round-trip");
            } else {
                println!("    ⚠️ Data corrupted but same length");
            }

            // Check if encoded string contains nul
            if encoded.contains('\0') {
                println!("    ❌ ENCODED STRING CONTAINS NUL!");
                println!("    This will fail when passed to git -m");
            }
        }
    }
}

/// Test the full compress_encode pipeline
#[test]
fn test_compress_encode_nul_safety() {
    let test_messages = vec![
        "add whirlpool command - three spiral methods with random selection",
        "Session wrap: wake ritual, bloom tagging, Honeypot check-in, 109 blooms",
        "Add iri command - bismuth staircase crystals with rainbow colors",
    ];

    println!("\n=== Testing compress_encode for Nul Safety ===\n");

    let registry = DictionaryRegistry::load_default().unwrap();

    for msg in test_messages {
        println!("Message: \"{}\"", msg);

        // Try compress_encode 100 times (random dictionary selection)
        let mut nul_found = false;
        for attempt in 1..=100 {
            let result = compress_encode(msg.as_bytes(), &registry).unwrap();

            // Check if encoded string contains nul
            let has_nul = result.encoded.contains('\0');

            if has_nul {
                println!("  ❌ Attempt {}: NUL FOUND!", attempt);
                println!("     Compression: {:?}", result.compress_algo);
                println!("     Dictionary: {}", result.dictionary_name);
                println!("     Encoded length: {}", result.encoded.len());

                // Show first 20 chars
                let preview: String = result
                    .encoded
                    .chars()
                    .take(20)
                    .map(|c| {
                        if c.is_control() {
                            format!("\\u{:04X}", c as u32)
                        } else {
                            c.to_string()
                        }
                    })
                    .collect();
                println!("     Preview: {}", preview);

                nul_found = true;
                // Don't panic yet - collect more data
                break;
            }
        }

        if !nul_found {
            println!("  ✓ All 100 attempts nul-free");
        }
    }
}

/// Focused test: ByteRange with start_codepoint=0
#[test]
fn test_byte_range_start_zero() {
    println!("\n=== Testing ByteRange with start_codepoint=0 ===\n");

    // Create ByteRange dictionary with start=0 (maps byte 0 -> U+0000)
    let dict = Dictionary::builder()
        .mode(EncodingMode::ByteRange)
        .start_codepoint(0)
        .build()
        .unwrap();

    // Test data with nul bytes
    let test_data = vec![0x48, 0x00, 0x65, 0x00, 0x6C, 0x6C, 0x6F]; // "H\0e\0llo"

    println!("Input bytes: {:?}", test_data);
    println!("Input length: {} bytes", test_data.len());

    let encoded = encode(&test_data, &dict);

    println!("\nEncoded: {:?}", encoded);
    println!("Encoded length: {} chars", encoded.chars().count());
    println!("Expected length: {} chars", test_data.len());

    // Check if nul is in encoded string
    if encoded.contains('\0') {
        println!("\n❌ CRITICAL: Encoded string contains nul character!");
        println!("This will cause 'nul byte found in provided data' error in git -m");

        // Show where the nuls are
        for (i, c) in encoded.chars().enumerate() {
            if c == '\0' {
                println!("  Nul at position {}", i);
            }
        }
    }

    // Try to decode
    let decoded = decode(&encoded, &dict).unwrap();
    println!("\nDecoded length: {} bytes", decoded.len());

    if decoded.len() != test_data.len() {
        println!(
            "❌ BYTES LOST: {} -> {} bytes",
            test_data.len(),
            decoded.len()
        );
    }

    if decoded == test_data {
        println!("✓ Round-trip preserves data (but contains nul in encoded form)");
    } else {
        println!("❌ Round-trip corrupted data");
    }
}

/// Test what happens when ByteRange maps to surrogate range
#[test]
fn test_byte_range_surrogate_range() {
    println!("\n=== Testing ByteRange Mapping to Surrogate Range ===\n");

    // Create dictionary with start=0xD700
    // This puts bytes 0x80-0xFF into surrogate range 0xD780-0xD7FF (INVALID)
    let dict = Dictionary::builder()
        .mode(EncodingMode::ByteRange)
        .start_codepoint(0xD700)
        .build()
        .unwrap();

    // Test data with bytes in problematic range
    let test_data: Vec<u8> = (0x00..=0xFF).collect(); // All possible bytes

    println!("Input: All 256 byte values (0x00-0xFF)");

    let encoded = encode(&test_data, &dict);
    println!("Encoded length: {} chars", encoded.chars().count());
    println!("Expected length: 256 chars (if no bytes dropped)");

    // Decode to see what we got back
    let decoded = decode(&encoded, &dict).unwrap();
    println!("Decoded length: {} bytes", decoded.len());

    let bytes_lost = test_data.len() - decoded.len();
    if bytes_lost > 0 {
        println!("\n❌ CRITICAL: {} BYTES LOST IN ENCODING!", bytes_lost);
        println!("ByteRange encoder silently drops bytes that map to invalid codepoints!");

        // Find which bytes were dropped
        let mut lost_bytes = Vec::new();
        for (i, &byte) in test_data.iter().enumerate() {
            if i >= decoded.len() || decoded[i] != byte {
                lost_bytes.push(byte);
            }
        }

        println!(
            "\nDropped bytes (first 20): {:02X?}",
            &lost_bytes[..20.min(lost_bytes.len())]
        );

        // Check if these map to invalid codepoints
        for &byte in &lost_bytes[..10] {
            let codepoint = 0xD700u32 + byte as u32;
            let is_surrogate = (0xD800..=0xDFFF).contains(&codepoint);
            println!(
                "  Byte 0x{:02X} -> U+{:04X} {}",
                byte,
                codepoint,
                if is_surrogate {
                    "(surrogate - INVALID)"
                } else {
                    ""
                }
            );
        }
    }
}
