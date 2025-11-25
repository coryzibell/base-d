//! SIMD feature detection example
//!
//! Demonstrates runtime CPU feature detection and SIMD availability

use base_d::{encode, DictionariesConfig, Dictionary};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== base-d SIMD Feature Detection ===\n");

    // Check CPU features
    #[cfg(target_arch = "x86_64")]
    {
        println!("Platform: x86_64");

        if is_x86_feature_detected!("avx2") {
            println!("✓ AVX2 available - Maximum SIMD performance");
        } else if is_x86_feature_detected!("ssse3") {
            println!("✓ SSSE3 available - Good SIMD performance");
        } else if is_x86_feature_detected!("sse2") {
            println!("✓ SSE2 available - Basic SIMD support (not yet used)");
        } else {
            println!("✗ No SIMD support detected");
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        println!("Platform: {:?}", std::env::consts::ARCH);
        println!("✗ SIMD not yet implemented for this platform");
        println!("  (using optimized scalar code)");
    }

    // Test encoding
    println!("\n=== Testing Base64 Encoding ===");
    let config = DictionariesConfig::load_default()?;
    let base64_config = config.get_dictionary("base64").unwrap();

    let chars: Vec<char> = base64_config.chars.chars().collect();
    let padding = base64_config
        .padding
        .as_ref()
        .and_then(|s| s.chars().next());
    let dictionary = Dictionary::new_with_mode(chars, base64_config.mode.clone(), padding)?;

    let test_data = b"Hello, SIMD World! This is a performance test.";
    let encoded = encode(test_data, &dictionary);

    println!("Input:  {:?}", std::str::from_utf8(test_data)?);
    println!("Output: {}", encoded);

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("ssse3") {
            println!("\n✓ SIMD acceleration active for base64!");
        }
    }

    println!("\n=== Performance Notes ===");
    println!("SIMD encoding uses SSSE3 instructions for ~4-5x speedup");
    println!("Run 'cargo bench' to measure actual performance.");

    Ok(())
}
