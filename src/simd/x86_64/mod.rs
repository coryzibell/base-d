//! x86_64 SIMD implementations
//!
//! This module provides SIMD-accelerated encoding and decoding for various
//! bit-width encodings on x86_64 platforms with SSSE3 support.

pub(crate) mod common;
mod specialized;

use crate::core::dictionary::Dictionary;
use crate::simd::alphabets::identify_base64_variant;
use specialized::base16::identify_hex_variant;

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
