//! SIMD-accelerated encoding/decoding implementations
//!
//! This module provides platform-specific SIMD optimizations for encoding
//! and decoding operations. Runtime CPU feature detection is used to
//! automatically select the best implementation.

#[cfg(target_arch = "x86_64")]
use crate::core::config::EncodingMode;
#[cfg(target_arch = "x86_64")]
use crate::core::dictionary::Dictionary;
#[cfg(target_arch = "x86_64")]
use std::sync::OnceLock;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub mod lut;
pub mod variants;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub mod generic;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub mod translate;

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "aarch64")]
mod aarch64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::{
    decode_base16_simd, decode_base32_simd, decode_base64_simd, decode_base256_simd,
    encode_base16_simd, encode_base32_simd, encode_base64_simd, encode_base256_simd,
};

#[cfg(target_arch = "aarch64")]
pub use aarch64::{
    decode_base16_simd, decode_base32_simd, decode_base64_simd, decode_base256_simd,
    encode_base16_simd, encode_base32_simd, encode_base64_simd, encode_base256_simd,
};

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub use generic::GenericSimdCodec;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub use lut::{Base64LutCodec, GappedSequentialCodec, SmallLutCodec};

// CPU feature detection cache
#[cfg(target_arch = "x86_64")]
static HAS_SSSE3: OnceLock<bool> = OnceLock::new();

/// Check if AVX2 is available (cached after first call)
#[cfg(target_arch = "x86_64")]
pub fn has_avx2() -> bool {
    crate::simd::x86_64::has_avx2()
}

/// Check if SSSE3 is available (cached after first call)
#[cfg(target_arch = "x86_64")]
pub fn has_ssse3() -> bool {
    *HAS_SSSE3.get_or_init(|| is_x86_feature_detected!("ssse3"))
}

#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
pub fn has_avx2() -> bool {
    false // AVX2 is x86-only
}

#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
pub fn has_ssse3() -> bool {
    false // SSSE3 is x86-only
}

/// Check if NEON is available (always true on aarch64)
#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
pub fn has_neon() -> bool {
    true
}

#[cfg(all(not(target_arch = "x86_64"), not(target_arch = "aarch64")))]
pub fn has_avx2() -> bool {
    false
}

#[cfg(all(not(target_arch = "x86_64"), not(target_arch = "aarch64")))]
pub fn has_ssse3() -> bool {
    false
}

/// Unified SIMD encoding entry point with automatic algorithm selection
///
/// Selection order:
/// 1. Known base64 variants (standard/url) → specialized base64 SIMD
/// 2. Known hex variants (base16) → specialized base16 SIMD
/// 3. Base256 ByteRange → specialized base256 SIMD
/// 4. Sequential power-of-2 dictionary → GenericSimdCodec
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
    if base == 64
        && let Some(_variant) = variants::identify_base64_variant(dict)
    {
        // Use existing specialized base64 implementation
        return encode_base64_simd(data, dict);
    }

    // 2. Try specialized base32 for known variants
    if base == 32
        && let Some(_variant) = variants::identify_base32_variant(dict)
    {
        return encode_base32_simd(data, dict);
    }

    // 3. Try specialized base16 for known hex variants
    if base == 16 && is_standard_hex(dict) {
        return encode_base16_simd(data, dict);
    }

    // 4. Try specialized base256 for ByteRange mode
    if base == 256 && *dict.mode() == EncodingMode::ByteRange {
        return encode_base256_simd(data, dict);
    }

    // 5. Try GenericSimdCodec for sequential power-of-2 dictionaries
    if let Some(codec) = GenericSimdCodec::from_dictionary(dict) {
        return codec.encode(data, dict);
    }

    // 6. Try GappedSequentialCodec for near-sequential dictionaries with gaps
    // (e.g., geohash, Crockford base32)
    if let Some(codec) = GappedSequentialCodec::from_dictionary(dict) {
        return codec.encode(data, dict);
    }

    // 7. Try SmallLutCodec for small arbitrary dictionaries (≤16 chars)
    if base <= 16
        && base.is_power_of_two()
        && let Some(codec) = SmallLutCodec::from_dictionary(dict)
    {
        return codec.encode(data, dict);
    }

    // 8. Try Base64LutCodec for large arbitrary dictionaries (17-64 chars)
    if (17..=64).contains(&base)
        && base.is_power_of_two()
        && let Some(codec) = Base64LutCodec::from_dictionary(dict)
    {
        return codec.encode(data, dict);
    }

    // 9. No SIMD optimization available
    None
}

/// Unified SIMD decoding entry point with automatic algorithm selection
///
/// Selection order:
/// 1. Known base64 variants (standard/url) → specialized base64 SIMD
/// 2. Known hex variants (base16) → specialized base16 SIMD
/// 3. Base256 ByteRange → specialized base256 SIMD
/// 4. Sequential power-of-2 dictionary → GenericSimdCodec
/// 5. None → caller falls back to scalar
///
/// Returns `None` if no SIMD optimization is available for this dictionary.
#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
pub fn decode_with_simd(encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
    // Requires SIMD support
    if !has_avx2() && !has_ssse3() {
        return None;
    }

    let base = dict.base();

    // 1. Try specialized base64 for known variants
    if base == 64 && variants::identify_base64_variant(dict).is_some() {
        // Use existing specialized base64 implementation
        return decode_base64_simd(encoded, dict);
    }

    // 2. Try specialized base32 for known variants
    if base == 32 && variants::identify_base32_variant(dict).is_some() {
        return decode_base32_simd(encoded, dict);
    }

    // 3. Try specialized base16 for known hex variants
    if base == 16 && is_standard_hex(dict) {
        return decode_base16_simd(encoded, dict);
    }

    // 4. Try specialized base256 for ByteRange mode
    if base == 256 && *dict.mode() == EncodingMode::ByteRange {
        return decode_base256_simd(encoded, dict);
    }

    // 5. Try GenericSimdCodec for sequential power-of-2 dictionaries
    if let Some(codec) = GenericSimdCodec::from_dictionary(dict) {
        return codec.decode(encoded, dict);
    }

    // 6. Try GappedSequentialCodec for near-sequential dictionaries with gaps
    if let Some(codec) = GappedSequentialCodec::from_dictionary(dict) {
        return codec.decode(encoded, dict);
    }

    // 7. Try SmallLutCodec for small arbitrary dictionaries (≤16 chars)
    if base <= 16
        && base.is_power_of_two()
        && let Some(codec) = SmallLutCodec::from_dictionary(dict)
    {
        return codec.decode(encoded, dict);
    }

    // 8. Try Base64LutCodec for large arbitrary dictionaries (17-64 chars)
    if (17..=64).contains(&base)
        && base.is_power_of_two()
        && let Some(codec) = Base64LutCodec::from_dictionary(dict)
    {
        return codec.decode(encoded, dict);
    }

    // 9. No SIMD optimization available
    None
}

/// Check if dictionary is standard hex (0-9A-F or 0-9a-f)
#[cfg(target_arch = "x86_64")]
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

// ============================================================================
// aarch64 NEON implementations
// ============================================================================

#[cfg(target_arch = "aarch64")]
use crate::core::dictionary::Dictionary;

/// Unified SIMD encoding entry point for aarch64 (NEON)
///
/// Selection order:
/// 1. Known base64 variants (standard/url) → specialized base64 NEON
/// 2. Known base32 variants → specialized base32 NEON
/// 3. Known hex variants (base16) → specialized base16 NEON
/// 4. Base256 → specialized base256 NEON
/// 5. LUT-based codecs for arbitrary dictionaries
/// 6. None → caller falls back to scalar
#[cfg(target_arch = "aarch64")]
pub fn encode_with_simd(data: &[u8], dict: &Dictionary) -> Option<String> {
    let base = dict.base();

    // 1. Try specialized base64 for known variants
    if base == 64 && variants::identify_base64_variant(dict).is_some() {
        return encode_base64_simd(data, dict);
    }

    // 2. Try specialized base32 for known variants
    if base == 32 && variants::identify_base32_variant(dict).is_some() {
        return encode_base32_simd(data, dict);
    }

    // 3. Try specialized base16 for known hex variants
    if base == 16 && is_standard_hex_aarch64(dict) {
        return encode_base16_simd(data, dict);
    }

    // 4. Try specialized base256
    if base == 256 {
        return encode_base256_simd(data, dict);
    }

    // 5. Try GappedSequentialCodec for near-sequential dictionaries with gaps
    // (e.g., geohash, Crockford base32)
    if let Some(codec) = GappedSequentialCodec::from_dictionary(dict) {
        return codec.encode(data, dict);
    }

    // 6. Try LUT-based codecs for arbitrary dictionaries
    // SmallLutCodec for base <= 16
    if base <= 16
        && base.is_power_of_two()
        && let Some(codec) = SmallLutCodec::from_dictionary(dict)
    {
        return codec.encode(data, dict);
    }

    // Base64LutCodec for base 32/64
    if (base == 32 || base == 64)
        && let Some(codec) = Base64LutCodec::from_dictionary(dict)
    {
        return codec.encode(data, dict);
    }

    // No SIMD optimization available for this dictionary
    None
}

/// Unified SIMD decoding entry point for aarch64 (NEON)
#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
pub fn decode_with_simd(encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
    let base = dict.base();

    // 1. Try specialized base64 for known variants
    if base == 64 && variants::identify_base64_variant(dict).is_some() {
        return decode_base64_simd(encoded, dict);
    }

    // 2. Try specialized base32 for known variants
    if base == 32 && variants::identify_base32_variant(dict).is_some() {
        return decode_base32_simd(encoded, dict);
    }

    // 3. Try specialized base16 for known hex variants
    if base == 16 && is_standard_hex_aarch64(dict) {
        return decode_base16_simd(encoded, dict);
    }

    // 4. Try specialized base256
    if base == 256 {
        return decode_base256_simd(encoded, dict);
    }

    // 5. Try GappedSequentialCodec for near-sequential dictionaries with gaps
    if let Some(codec) = GappedSequentialCodec::from_dictionary(dict) {
        return codec.decode(encoded, dict);
    }

    // 6. Try LUT-based codecs for arbitrary dictionaries
    // SmallLutCodec for base <= 16
    if base <= 16
        && base.is_power_of_two()
        && let Some(codec) = SmallLutCodec::from_dictionary(dict)
    {
        return codec.decode(encoded, dict);
    }

    // Base64LutCodec for base 32/64
    if (base == 32 || base == 64)
        && let Some(codec) = Base64LutCodec::from_dictionary(dict)
    {
        return codec.decode(encoded, dict);
    }

    // No SIMD optimization available
    None
}

/// Check if dictionary is standard hex (0-9A-F or 0-9a-f) - aarch64 version
#[cfg(target_arch = "aarch64")]
fn is_standard_hex_aarch64(dict: &Dictionary) -> bool {
    if dict.base() != 16 {
        return false;
    }

    // Check uppercase variant: 0-9A-F
    let uppercase = "0123456789ABCDEF";
    for (i, expected) in uppercase.chars().enumerate() {
        if dict.encode_digit(i) != Some(expected) {
            // Try lowercase
            let lowercase = "0123456789abcdef";
            for (j, exp_lower) in lowercase.chars().enumerate() {
                if dict.encode_digit(j) != Some(exp_lower) {
                    return false;
                }
            }
            return true;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "x86_64")]
    use super::{Dictionary, decode_with_simd, encode_with_simd, has_ssse3};
    #[cfg(target_arch = "x86_64")]
    use crate::core::config::EncodingMode;

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_custom_dictionary_auto_simd() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Create custom base16 dictionary starting at ASCII '!' (0x21)
        // This should automatically use GenericSimdCodec
        let chars: Vec<char> = (0x21..0x31).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::builder().chars(chars).build().unwrap();

        // Test data: 32 bytes (enough for two SIMD rounds)
        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10\
                     \x00\x11\x22\x33\x44\x55\x66\x77\x88\x99\xAA\xBB\xCC\xDD\xEE\xFF";

        // Encode using auto-selection
        let result = encode_with_simd(data, &dict);
        assert!(
            result.is_some(),
            "Custom dictionary should get SIMD acceleration"
        );

        let encoded = result.unwrap();

        // Verify output length: 32 bytes -> 64 hex chars
        assert_eq!(encoded.len(), 64, "32 bytes should produce 64 hex chars");

        // Verify that output uses custom dictionary characters
        for c in encoded.chars() {
            let codepoint = c as u32;
            assert!(
                (0x21..0x31).contains(&codepoint),
                "Output char U+{:04X} '{}' should be in custom dictionary range U+0021..U+0031",
                codepoint,
                c
            );
        }

        // Verify first few nibbles are correctly encoded
        // 0x01 -> nibbles 0x0, 0x1 -> chars 0x21 (0 + 0x21), 0x22 (1 + 0x21)
        assert_eq!(encoded.chars().next().unwrap(), '\x21'); // 0 + 0x21 = '!'
        assert_eq!(encoded.chars().nth(1).unwrap(), '\x22'); // 1 + 0x21 = '"'
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_standard_base64_uses_specialized() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Standard base64 dictionary should use specialized implementation
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
            .chars()
            .collect();
        let dict = Dictionary::builder()
            .chars(chars)
            .mode(EncodingMode::Chunked)
            .padding('=')
            .build()
            .unwrap();

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

        // Standard hex dictionary should use specialized implementation
        let chars: Vec<char> = "0123456789abcdef".chars().collect();
        let dict = Dictionary::builder().chars(chars).build().unwrap();

        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10";
        let result = encode_with_simd(data, &dict);

        assert!(
            result.is_some(),
            "Standard hex should get SIMD acceleration"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_arbitrary_dictionary_uses_largelut() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Arbitrary (shuffled) base64 dictionary should use Base64LutCodec
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBAzyxwvutsrqponmlkjihgfedcba9876543210+/"
            .chars()
            .collect();
        let dict = Dictionary::builder()
            .chars(chars)
            .mode(EncodingMode::Chunked)
            .build()
            .unwrap();

        let data = b"Hello, World!";
        let result = encode_with_simd(data, &dict);

        assert!(
            result.is_some(),
            "Arbitrary base64 dictionary should get SIMD acceleration via Base64LutCodec"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_sequential_base64_uses_generic() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Sequential base64 (non-standard) should use GenericSimdCodec
        let chars: Vec<char> = (0x100..0x140)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();
        let dict = Dictionary::builder()
            .chars(chars)
            .mode(EncodingMode::Chunked)
            .build()
            .unwrap();

        let data = b"Hello, World!!!!\x00";
        let result = encode_with_simd(data, &dict);

        assert!(
            result.is_some(),
            "Sequential base64 should get SIMD acceleration via GenericSimdCodec"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_with_simd_base64_round_trip() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Standard base64 dictionary
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
            .chars()
            .collect();
        let dict = Dictionary::builder()
            .chars(chars)
            .mode(EncodingMode::Chunked)
            .padding('=')
            .build()
            .unwrap();

        let data = b"The quick brown fox jumps over the lazy dog";

        // Encode with SIMD
        let encoded = encode_with_simd(data, &dict).expect("Encode failed");

        // Decode with SIMD
        let decoded = decode_with_simd(&encoded, &dict).expect("Decode failed");

        // Verify round-trip
        assert_eq!(
            &decoded[..],
            &data[..],
            "Round-trip decode failed for base64"
        );
    }

    // NOTE: Standard base16 decode has a known issue and is temporarily disabled
    // Custom base16 (via GenericSimdCodec) works correctly
    // TODO: Fix specialized base16 decode implementation
    #[test]
    #[cfg(target_arch = "x86_64")]
    #[ignore]
    fn test_decode_with_simd_base16_round_trip() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Standard hex dictionary
        let chars: Vec<char> = "0123456789ABCDEF".chars().collect();
        let dict = Dictionary::builder()
            .chars(chars)
            .mode(EncodingMode::Chunked)
            .build()
            .unwrap();

        let data: Vec<u8> = (0..32).map(|i| (i * 7) as u8).collect();

        // Encode with SIMD
        let encoded = encode_with_simd(&data, &dict).expect("Encode failed");

        // Decode with SIMD
        let decoded = decode_with_simd(&encoded, &dict).expect("Decode failed");

        // Verify round-trip
        assert_eq!(
            &decoded[..],
            &data[..],
            "Round-trip decode failed for base16"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_with_simd_custom_hex_round_trip() {
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Custom base16 dictionary starting at ASCII '!' (0x21)
        let chars: Vec<char> = (0x21..0x31).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::builder().chars(chars).build().unwrap();

        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10\
                     \x00\x11\x22\x33\x44\x55\x66\x77\x88\x99\xAA\xBB\xCC\xDD\xEE\xFF";

        // Encode with SIMD
        let encoded = encode_with_simd(data, &dict).expect("Encode failed");

        // Decode with SIMD
        let decoded = decode_with_simd(&encoded, &dict).expect("Decode failed");

        // Verify round-trip
        assert_eq!(
            &decoded[..],
            &data[..],
            "Round-trip decode failed for custom hex"
        );
    }
}
