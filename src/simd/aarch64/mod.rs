//! ARM NEON (aarch64) SIMD implementations
//!
//! This module provides SIMD-accelerated encoding and decoding for various
//! bit-width encodings on aarch64 platforms with NEON support.
//!
//! NEON is mandatory on aarch64, so all ARM64 CPUs support these instructions.

mod specialized;

use crate::core::dictionary::Dictionary;
use crate::simd::alphabets::identify_base64_variant;
use specialized::base16::identify_hex_variant;

/// Check if NEON is available
///
/// On aarch64, NEON is mandatory per the ARM architecture specification.
/// This function always returns true on aarch64 targets.
#[inline]
pub fn has_neon() -> bool {
    cfg!(target_arch = "aarch64")
}

/// Public API for SIMD base64 encoding on aarch64
///
/// This function dispatches to the NEON implementation for base64 encoding.
#[cfg(target_arch = "aarch64")]
pub fn encode_base64_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize standard base64 (6 bits per char)
    if dictionary.base() != 64 {
        return None;
    }

    // Identify which base64 variant this is
    let variant = identify_base64_variant(dictionary)?;

    // NEON is always available on aarch64
    if !has_neon() {
        return None;
    }

    specialized::base64::encode(data, dictionary, variant)
}

/// Public API for SIMD base64 decoding on aarch64
#[cfg(target_arch = "aarch64")]
pub fn decode_base64_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base64 with known variants
    if dictionary.base() != 64 {
        return None;
    }

    let variant = identify_base64_variant(dictionary)?;

    // NEON is always available on aarch64
    if !has_neon() {
        return None;
    }

    // Minimum 16 bytes for SIMD processing
    if encoded.len() < 16 {
        return None;
    }

    specialized::base64::decode(encoded, variant)
}

/// Public API for SIMD base16/hex encoding on aarch64
///
/// Currently returns None (placeholder implementation).
#[cfg(target_arch = "aarch64")]
pub fn encode_base16_simd(_data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize base16 (hex)
    if dictionary.base() != 16 {
        return None;
    }

    // Identify which hex variant this is
    let _variant = identify_hex_variant(dictionary)?;

    // NEON is always available on aarch64
    if !has_neon() {
        return None;
    }

    // TODO: Implement NEON base16 encoding
    None
}

/// Public API for SIMD base16/hex decoding on aarch64
///
/// Currently returns None (placeholder implementation).
#[cfg(target_arch = "aarch64")]
pub fn decode_base16_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base16 with known variants
    if dictionary.base() != 16 {
        return None;
    }

    let _variant = identify_hex_variant(dictionary)?;

    // NEON is always available on aarch64
    if !has_neon() {
        return None;
    }

    // Minimum 32 bytes for SIMD processing (16 output bytes)
    if encoded.len() < 32 {
        return None;
    }

    // TODO: Implement NEON base16 decoding
    None
}

/// Public API for SIMD base256 encoding on aarch64
///
/// Dispatches to NEON implementation for base256 encoding.
#[cfg(target_arch = "aarch64")]
pub fn encode_base256_simd(data: &[u8], dictionary: &Dictionary) -> Option<String> {
    // Only optimize base256 (8-bit encoding)
    if dictionary.base() != 256 {
        return None;
    }

    // NEON is always available on aarch64
    if !has_neon() {
        return None;
    }

    specialized::base256::encode(data, dictionary)
}

/// Public API for SIMD base256 decoding on aarch64
///
/// Dispatches to NEON implementation for base256 decoding.
#[cfg(target_arch = "aarch64")]
pub fn decode_base256_simd(encoded: &str, dictionary: &Dictionary) -> Option<Vec<u8>> {
    // Only optimize base256 (8-bit encoding)
    if dictionary.base() != 256 {
        return None;
    }

    // NEON is always available on aarch64
    if !has_neon() {
        return None;
    }

    specialized::base256::decode(encoded, dictionary)
}
