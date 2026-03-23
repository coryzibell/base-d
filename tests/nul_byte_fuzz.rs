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

/// Focused test: ByteRange with start_codepoint=0 should be REJECTED
///
/// Previously this would succeed and produce strings containing NUL (U+0000)
/// and C1 control characters, causing garbled git commit messages.
/// After the fix, the Dictionary builder rejects unsafe start_codepoints.
#[test]
fn test_byte_range_start_zero() {
    println!("\n=== Testing ByteRange with start_codepoint=0 (should be rejected) ===\n");

    // Create ByteRange dictionary with start=0 (maps byte 0 -> U+0000)
    // This MUST fail because the range U+0000..U+00FF includes NUL and C1 controls
    let result = Dictionary::builder()
        .mode(EncodingMode::ByteRange)
        .start_codepoint(0)
        .build();

    assert!(
        result.is_err(),
        "ByteRange with start_codepoint=0 should be rejected (maps to NUL and C1 controls)"
    );

    let err = result.unwrap_err();
    println!("Correctly rejected: {}", err);
    assert!(
        err.contains("Unsafe ByteRange"),
        "Error message should mention unsafe ByteRange: {}",
        err
    );
}

/// Test that ByteRange with start_codepoint overlapping surrogates is REJECTED
///
/// With start=0xD701, end = 0xD701+255 = 0xD800, which overlaps the surrogate
/// range (U+D800-U+DFFF). The Dictionary builder must reject this.
#[test]
fn test_byte_range_surrogate_range() {
    println!(
        "\n=== Testing ByteRange with start_codepoint overlapping surrogates (should be rejected) ===\n"
    );

    // Create ByteRange dictionary with start=0xD701 (end = 0xD800, overlaps surrogate start)
    // This MUST fail because byte 0xFF would map to U+D800 (first surrogate)
    let result = Dictionary::builder()
        .mode(EncodingMode::ByteRange)
        .start_codepoint(0xD701)
        .build();

    assert!(
        result.is_err(),
        "ByteRange with start_codepoint=0xD701 should be rejected (end 0xD800 overlaps surrogates)"
    );

    let err = result.unwrap_err();
    println!("Correctly rejected: {}", err);
    assert!(
        err.contains("Unsafe ByteRange"),
        "Error message should mention unsafe ByteRange: {}",
        err
    );
}
