//! x86_64 SIMD implementations
//!
//! This module provides SIMD-accelerated encoding and decoding for various
//! bit-width encodings on x86_64 platforms with SSSE3 and AVX2 support.

pub(crate) mod common;
mod specialized;

use crate::core::dictionary::Dictionary;
use crate::simd::variants::{identify_base32_variant, identify_base64_variant};
use specialized::base16::identify_hex_variant;
use std::sync::OnceLock;

/// SIMD capability levels supported on x86_64
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SimdLevel {
    /// AVX2 support (256-bit SIMD, requires Haswell+/Excavator+, 2013+)
    Avx2,
    /// SSSE3 support (128-bit SIMD with pshufb, requires Nehalem+/Bulldozer+, 2008+)
    Ssse3,
    /// No SIMD support
    None,
}

/// Cached SIMD level detection result
#[allow(dead_code)]
static SIMD_LEVEL: OnceLock<SimdLevel> = OnceLock::new();

/// Detect available SIMD features on x86_64
///
/// Checks for AVX2 first, then SSSE3, using CPUID via `is_x86_feature_detected!()`.
/// Result is cached in OnceLock for zero-cost subsequent calls.
#[inline]
#[allow(dead_code)]
fn detect_simd_level() -> SimdLevel {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            SimdLevel::Avx2
        } else if is_x86_feature_detected!("ssse3") {
            SimdLevel::Ssse3
        } else {
            SimdLevel::None
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        SimdLevel::None
    }
}

/// Get the cached SIMD level for this CPU
///
/// The first call performs CPUID detection. Subsequent calls return the cached result
/// with negligible overhead.
#[inline]
#[allow(dead_code)]
pub fn simd_level() -> SimdLevel {
    *SIMD_LEVEL.get_or_init(detect_simd_level)
}

/// Check if AVX2 is available
#[inline]
#[allow(dead_code)]
pub fn has_avx2() -> bool {
    matches!(simd_level(), SimdLevel::Avx2)
}

/// Check if SSSE3 is available (legacy function, check for AVX2 or SSSE3)
///
/// Returns true if either SSSE3 or AVX2 is available, since AVX2 implies SSSE3 support.
#[inline]
#[allow(dead_code)]
pub fn has_ssse3() -> bool {
    matches!(simd_level(), SimdLevel::Ssse3 | SimdLevel::Avx2)
}

/// Public API for SIMD base64 encoding
///
/// This function dispatches to the appropriate SIMD implementation based on
/// the dictionary's bit-width. Currently only base64 (6-bit) is supported.
#[cfg(target_arch = "x86_64")]
pub fn encode_base64_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize standard base64 (6 bits per char)
    if dictionary.base() != 64 {
        return None;
    }

    // Identify which base64 variant this is
    let variant = identify_base64_variant(dictionary)?;

    // Need SSSE3 for pshufb
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base64::encode(data, dictionary, variant)
}

/// Public API for SIMD base64 decoding
#[cfg(target_arch = "x86_64")]
pub fn decode_base64_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base64 with known variants
    if dictionary.base() != 64 {
        return None;
    }

    let variant = identify_base64_variant(dictionary)?;

    // Need SSSE3 for pshufb
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Minimum 16 bytes for SIMD processing
    if encoded.len() < 16 {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base64::decode(encoded, variant)
}

/// Public API for SIMD base16/hex encoding
#[cfg(target_arch = "x86_64")]
pub fn encode_base16_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize base16 (hex)
    if dictionary.base() != 16 {
        return None;
    }

    // Identify which hex variant this is
    let variant = identify_hex_variant(dictionary)?;

    // Need SSSE3 for pshufb
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base16::encode(data, dictionary, variant)
}

/// Public API for SIMD base16/hex decoding
#[cfg(target_arch = "x86_64")]
pub fn decode_base16_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base16 with known variants
    if dictionary.base() != 16 {
        return None;
    }

    let variant = identify_hex_variant(dictionary)?;

    // Need SSSE3 for pshufb
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Minimum 32 bytes for SIMD processing (16 output bytes)
    if encoded.len() < 32 {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base16::decode(encoded, variant)
}

/// Public API for SIMD base256 encoding
#[cfg(target_arch = "x86_64")]
pub fn encode_base256_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize base256 (8-bit encoding)
    if dictionary.base() != 256 {
        return None;
    }

    // Need SSSE3 for efficient SIMD loads/stores
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base256::encode(data, dictionary)
}

/// Public API for SIMD base256 decoding
#[cfg(target_arch = "x86_64")]
pub fn decode_base256_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base256 (8-bit encoding)
    if dictionary.base() != 256 {
        return None;
    }

    // Need SSSE3 for efficient SIMD loads/stores
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Minimum 16 bytes for SIMD processing
    if encoded.len() < 16 {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base256::decode(encoded, dictionary)
}

/// Public API for SIMD base32 encoding
#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
pub fn encode_base32_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize base32 (5-bit encoding)
    if dictionary.base() != 32 {
        return None;
    }

    // Identify which base32 variant this is
    let variant = identify_base32_variant(dictionary)?;

    // Need SSSE3 for pshufb
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base32::encode(data, dictionary, variant)
}

/// Public API for SIMD base32 decoding
#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
pub fn decode_base32_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base32 with known variants
    if dictionary.base() != 32 {
        return None;
    }

    let variant = identify_base32_variant(dictionary)?;

    // Need SSSE3 for pshufb
    if !is_x86_feature_detected!("ssse3") {
        return None;
    }

    // Minimum 16 bytes for SIMD processing
    if encoded.len() < 16 {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base32::decode(encoded, variant)
}
