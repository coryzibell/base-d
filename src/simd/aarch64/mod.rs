//! aarch64 SIMD implementations
//!
//! This module provides SIMD-accelerated encoding and decoding for various
//! bit-width encodings on aarch64 platforms with NEON support.

#[cfg(target_arch = "aarch64")]
pub(crate) mod common;

#[cfg(target_arch = "aarch64")]
mod specialized;

#[cfg(target_arch = "aarch64")]
use crate::core::dictionary::Dictionary;
#[cfg(target_arch = "aarch64")]
use crate::simd::variants::{identify_base32_variant, identify_base64_variant};
#[cfg(target_arch = "aarch64")]
use specialized::base16::identify_hex_variant;

/// Check if NEON is available
///
/// On aarch64, NEON is always available as part of the baseline architecture.
/// This function exists to maintain API compatibility with x86_64.
#[cfg(target_arch = "aarch64")]
#[inline]
#[allow(dead_code)]
pub fn has_neon() -> bool {
    true
}

/// Public API for SIMD base64 encoding
///
/// This function dispatches to the appropriate SIMD implementation based on
/// the dictionary's bit-width. Currently only base64 (6-bit) is supported.
#[cfg(target_arch = "aarch64")]
pub fn encode_base64_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize standard base64 (6 bits per char)
    if dictionary.base() != 64 {
        return None;
    }

    // Identify which base64 variant this is
    let variant = identify_base64_variant(dictionary)?;

    // Dispatch to specialized implementation
    specialized::base64::encode(data, dictionary, variant)
}

/// Public API for SIMD base64 decoding
#[cfg(target_arch = "aarch64")]
pub fn decode_base64_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base64 with known variants
    if dictionary.base() != 64 {
        return None;
    }

    let variant = identify_base64_variant(dictionary)?;

    // Minimum 16 bytes for SIMD processing
    if encoded.len() < 16 {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base64::decode(encoded, variant)
}

/// Public API for SIMD base16/hex encoding
#[cfg(target_arch = "aarch64")]
pub fn encode_base16_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize base16 (hex)
    if dictionary.base() != 16 {
        return None;
    }

    // Identify which hex variant this is
    let variant = identify_hex_variant(dictionary)?;

    // Dispatch to specialized implementation
    specialized::base16::encode(data, dictionary, variant)
}

/// Public API for SIMD base16/hex decoding
#[cfg(target_arch = "aarch64")]
pub fn decode_base16_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base16 with known variants
    if dictionary.base() != 16 {
        return None;
    }

    let variant = identify_hex_variant(dictionary)?;

    // Minimum 32 bytes for SIMD processing (16 output bytes)
    if encoded.len() < 32 {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base16::decode(encoded, variant)
}

/// Public API for SIMD base256 encoding
#[cfg(target_arch = "aarch64")]
pub fn encode_base256_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize base256 (8-bit encoding)
    if dictionary.base() != 256 {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base256::encode(data, dictionary)
}

/// Public API for SIMD base256 decoding
#[cfg(target_arch = "aarch64")]
pub fn decode_base256_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base256 (8-bit encoding)
    if dictionary.base() != 256 {
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
#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
pub fn encode_base32_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize base32 (5-bit encoding)
    if dictionary.base() != 32 {
        return None;
    }

    // Identify which base32 variant this is
    let variant = identify_base32_variant(dictionary)?;

    // Dispatch to specialized implementation
    specialized::base32::encode(data, dictionary, variant)
}

/// Public API for SIMD base32 decoding
#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
pub fn decode_base32_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base32 with known variants
    if dictionary.base() != 32 {
        return None;
    }

    let variant = identify_base32_variant(dictionary)?;

    // Minimum 16 bytes for SIMD processing
    if encoded.len() < 16 {
        return None;
    }

    // Dispatch to specialized implementation
    specialized::base32::decode(encoded, variant)
}
