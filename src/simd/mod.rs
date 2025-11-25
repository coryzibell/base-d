//! SIMD-accelerated encoding/decoding implementations
//!
//! This module provides platform-specific SIMD optimizations for encoding
//! and decoding operations. Runtime CPU feature detection is used to
//! automatically select the best implementation.

use crate::core::config::EncodingMode;
use crate::core::dictionary::Dictionary;
use std::sync::OnceLock;

pub mod alphabets;
pub mod generic;
pub mod translate;

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::{
    decode_base16_simd, decode_base256_simd, decode_base64_simd, encode_base16_simd,
    encode_base256_simd, encode_base64_simd,
};

pub use generic::GenericSimdEncoder;

// CPU feature detection cache
static HAS_AVX2: OnceLock<bool> = OnceLock::new();

#[cfg(target_arch = "x86_64")]
static HAS_SSSE3: OnceLock<bool> = OnceLock::new();

/// Check if AVX2 is available (cached after first call)
#[cfg(target_arch = "x86_64")]
pub fn has_avx2() -> bool {
    *HAS_AVX2.get_or_init(|| is_x86_feature_detected!("avx2"))
}

/// Check if SSSE3 is available (cached after first call)
#[cfg(target_arch = "x86_64")]
pub fn has_ssse3() -> bool {
    *HAS_SSSE3.get_or_init(|| is_x86_feature_detected!("ssse3"))
}

#[cfg(not(target_arch = "x86_64"))]
pub fn has_avx2() -> bool {
    false
}

#[cfg(not(target_arch = "x86_64"))]
pub fn has_ssse3() -> bool {
    false
}

/// Unified SIMD encoding entry point with automatic algorithm selection
///
/// Selection order:
/// 1. Known base64 variants (standard/url) → specialized base64 SIMD
/// 2. Known hex variants (base16) → specialized base16 SIMD
/// 3. Base256 ByteRange → specialized base256 SIMD
/// 4. Sequential power-of-2 alphabet → GenericSimdEncoder
/// 5. None → caller falls back to scalar
///
/// Returns `None` if no SIMD optimization is available for this dictionary.
#[cfg(target_arch = "x86_64")]
pub fn encode_with_simd(data: &[u8], dict: &Dictionary) -> Option<String> {
    // Requires SIMD support
    if !has_avx2() && !has_ssse3() {
        return None;
    }

    let base = dict.base();

    // 1. Try specialized base64 for known variants
    if base == 64 {
        if let Some(_variant) = alphabets::identify_base64_variant(dict) {
            // Use existing specialized base64 implementation
            return encode_base64_simd(data, dict);
        }
    }

    // 2. Try specialized base16 for known hex variants
    if base == 16 {
        // Check if this matches uppercase or lowercase hex
        if is_standard_hex(dict) {
            return encode_base16_simd(data, dict);
        }
    }

    // 3. Try specialized base256 for ByteRange mode
    if base == 256 && *dict.mode() == EncodingMode::ByteRange {
        return encode_base256_simd(data, dict);
    }

    // 4. Try GenericSimdEncoder for sequential power-of-2 alphabets
    if let Some(encoder) = GenericSimdEncoder::from_dictionary(dict) {
        return encoder.encode(data, dict);
    }

    // 5. No SIMD optimization available
    None
}

/// Fallback for non-x86_64 platforms
#[cfg(not(target_arch = "x86_64"))]
pub fn encode_with_simd(_data: &[u8], _dict: &Dictionary) -> Option<String> {
    None
}

/// Check if dictionary is standard hex (0-9A-F or 0-9a-f)
fn is_standard_hex(dict: &Dictionary) -> bool {
    if dict.base() != 16 {
        return false;
    }

    // Check uppercase variant: 0-9A-F
    let uppercase = "0123456789ABCDEF";
    let mut matches_upper = true;
    for (i, expected) in uppercase.chars().enumerate() {
        if dict.encode_digit(i) != Some(expected) {
            matches_upper = false;
            break;
        }
    }
    if matches_upper {
        return true;
    }

    // Check lowercase variant: 0-9a-f
    let lowercase = "0123456789abcdef";
    let mut matches_lower = true;
    for (i, expected) in lowercase.chars().enumerate() {
        if dict.encode_digit(i) != Some(expected) {
            matches_lower = false;
            break;
        }
    }
    matches_lower
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::EncodingMode;

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_custom_alphabet_auto_simd() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Create custom base16 alphabet starting at ASCII '!' (0x21)
        // This should automatically use GenericSimdEncoder
        let chars: Vec<char> = (0x21..0x31).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        // Test data: 32 bytes (enough for two SIMD rounds)
        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10\
                     \x00\x11\x22\x33\x44\x55\x66\x77\x88\x99\xAA\xBB\xCC\xDD\xEE\xFF";

        // Encode using auto-selection
        let result = encode_with_simd(data, &dict);
        assert!(
            result.is_some(),
            "Custom alphabet should get SIMD acceleration"
        );

        let encoded = result.unwrap();

        // Verify output length: 32 bytes -> 64 hex chars
        assert_eq!(encoded.len(), 64, "32 bytes should produce 64 hex chars");

        // Verify that output uses custom alphabet characters
        for c in encoded.chars() {
            let codepoint = c as u32;
            assert!(
                codepoint >= 0x21 && codepoint < 0x31,
                "Output char U+{:04X} '{}' should be in custom alphabet range U+0021..U+0031",
                codepoint,
                c
            );
        }

        // Verify first few nibbles are correctly encoded
        // 0x01 -> nibbles 0x0, 0x1 -> chars 0x21 (0 + 0x21), 0x22 (1 + 0x21)
        assert_eq!(encoded.chars().nth(0).unwrap(), '\x21'); // 0 + 0x21 = '!'
        assert_eq!(encoded.chars().nth(1).unwrap(), '\x22'); // 1 + 0x21 = '"'
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_standard_base64_uses_specialized() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Standard base64 alphabet should use specialized implementation
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
            .chars()
            .collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, Some('=')).unwrap();

        let data = b"Hello, World!";
        let result = encode_with_simd(data, &dict);

        assert!(
            result.is_some(),
            "Standard base64 should get SIMD acceleration"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_standard_hex_uses_specialized() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Standard hex alphabet should use specialized implementation
        let chars: Vec<char> = "0123456789abcdef".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10";
        let result = encode_with_simd(data, &dict);

        assert!(
            result.is_some(),
            "Standard hex should get SIMD acceleration"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_arbitrary_alphabet_falls_back() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Arbitrary (shuffled) alphabet should return None (no SIMD)
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBAzyxwvutsrqponmlkjihgfedcba9876543210+/"
            .chars()
            .collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();

        let data = b"Hello, World!";
        let result = encode_with_simd(data, &dict);

        assert!(
            result.is_none(),
            "Arbitrary alphabet should not get SIMD acceleration"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_sequential_base64_uses_generic() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Sequential base64 (non-standard) should use GenericSimdEncoder
        let chars: Vec<char> = (0x100..0x140)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();

        let data = b"Hello, World!!!!\x00";
        let result = encode_with_simd(data, &dict);

        assert!(
            result.is_some(),
            "Sequential base64 should get SIMD acceleration via GenericSimdEncoder"
        );
    }
}
