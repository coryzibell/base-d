//! Benchmarking utilities for comparing encoding paths.
//!
//! This module exposes internal encoding paths for performance comparison:
//! - Scalar: Pure Rust, no SIMD
//! - LUT: SIMD with runtime lookup tables
//! - Specialized: Hardcoded SIMD for known dictionaries
//!
//! # Example
//!
//! ```ignore
//! use base_d::bench::{EncodingPath, encode_with_path, detect_available_paths};
//!
//! let dict = get_dictionary("base64");
//! let paths = detect_available_paths(&dict);
//!
//! for path in paths {
//!     let result = encode_with_path(data, &dict, path);
//! }
//! ```

use crate::EncodingMode;
use crate::core::dictionary::Dictionary;
use crate::encoders::algorithms::{DecodeError, byte_range, radix};

#[cfg(feature = "simd")]
use crate::simd;

/// Available encoding paths for benchmarking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncodingPath {
    /// Pure scalar implementation (no SIMD)
    Scalar,
    /// SIMD with runtime LUT construction
    Lut,
    /// Hardcoded SIMD for known RFC dictionaries
    Specialized,
}

impl std::fmt::Display for EncodingPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodingPath::Scalar => write!(f, "Scalar"),
            EncodingPath::Lut => write!(f, "LUT"),
            EncodingPath::Specialized => write!(f, "Specialized"),
        }
    }
}

/// Platform capabilities for SIMD.
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    pub arch: &'static str,
    pub simd_features: Vec<&'static str>,
}

impl PlatformInfo {
    /// Detect current platform capabilities.
    pub fn detect() -> Self {
        let arch = std::env::consts::ARCH;
        let mut simd_features = Vec::new();

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx512vbmi") {
                simd_features.push("AVX-512 VBMI");
            }
            if is_x86_feature_detected!("avx2") {
                simd_features.push("AVX2");
            }
            if is_x86_feature_detected!("ssse3") {
                simd_features.push("SSSE3");
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // NEON is always available on aarch64
            simd_features.push("NEON");
        }

        PlatformInfo {
            arch,
            simd_features,
        }
    }

    /// Format as display string.
    pub fn display(&self) -> String {
        if self.simd_features.is_empty() {
            self.arch.to_string()
        } else {
            format!("{} ({})", self.arch, self.simd_features.join(", "))
        }
    }
}

/// Information about a dictionary's benchmark capabilities.
#[derive(Debug, Clone)]
pub struct DictionaryBenchInfo {
    pub name: String,
    pub base: usize,
    pub mode: EncodingMode,
    pub available_paths: Vec<EncodingPath>,
    pub supports_streaming: bool,
}

/// Detect which encoding paths are available for a dictionary.
pub fn detect_available_paths(dict: &Dictionary) -> Vec<EncodingPath> {
    let mut paths = vec![EncodingPath::Scalar]; // Scalar always available

    #[cfg(feature = "simd")]
    {
        let base = dict.base();
        let mode = dict.mode();

        // Check if LUT path is available (power-of-2 base, ASCII chars)
        if base.is_power_of_two() && base <= 256 {
            // Check if all chars are ASCII
            let all_ascii = (0..base).all(|i| {
                dict.encode_digit(i)
                    .map(|c| (c as u32) < 128)
                    .unwrap_or(false)
            });

            if all_ascii && matches!(mode, EncodingMode::Chunked) {
                paths.push(EncodingPath::Lut);
            }
        }

        // Check if specialized path is available
        if is_specialized_available(dict) {
            paths.push(EncodingPath::Specialized);
        }
    }

    paths
}

/// Check if a specialized SIMD path exists for this dictionary.
#[cfg(feature = "simd")]
fn is_specialized_available(dict: &Dictionary) -> bool {
    use crate::simd::variants::{identify_base32_variant, identify_base64_variant};

    let base = dict.base();

    match base {
        16 => {
            // Check if it's standard hex (uppercase or lowercase)
            let first_char = dict.encode_digit(10); // 'A' or 'a' position
            matches!(first_char, Some('A') | Some('a'))
        }
        32 => identify_base32_variant(dict).is_some(),
        64 => identify_base64_variant(dict).is_some(),
        256 => matches!(dict.mode(), EncodingMode::Chunked | EncodingMode::ByteRange),
        _ => false,
    }
}

#[cfg(not(feature = "simd"))]
fn is_specialized_available(_dict: &Dictionary) -> bool {
    false
}

/// Encode using a specific path (for benchmarking).
///
/// Returns `None` if the path is not available for this dictionary.
pub fn encode_with_path(data: &[u8], dict: &Dictionary, path: EncodingPath) -> Option<String> {
    match path {
        EncodingPath::Scalar => Some(encode_scalar(data, dict)),
        EncodingPath::Lut => encode_lut(data, dict),
        EncodingPath::Specialized => encode_specialized(data, dict),
    }
}

/// Decode using a specific path (for benchmarking).
///
/// Returns `None` if the path is not available for this dictionary.
pub fn decode_with_path(encoded: &str, dict: &Dictionary, path: EncodingPath) -> Option<Vec<u8>> {
    match path {
        EncodingPath::Scalar => decode_scalar(encoded, dict).ok(),
        EncodingPath::Lut => decode_lut(encoded, dict),
        EncodingPath::Specialized => decode_specialized(encoded, dict),
    }
}

/// Pure scalar encoding (no SIMD).
fn encode_scalar(data: &[u8], dict: &Dictionary) -> String {
    match dict.mode() {
        EncodingMode::Radix => radix::encode(data, dict),
        EncodingMode::Chunked => encode_chunked_scalar(data, dict),
        EncodingMode::ByteRange => byte_range::encode_byte_range(data, dict),
    }
}

/// Pure scalar decoding (no SIMD).
fn decode_scalar(encoded: &str, dict: &Dictionary) -> Result<Vec<u8>, crate::DecodeError> {
    match dict.mode() {
        EncodingMode::Radix => radix::decode(encoded, dict),
        EncodingMode::Chunked => decode_chunked_scalar(encoded, dict),
        EncodingMode::ByteRange => byte_range::decode_byte_range(encoded, dict),
    }
}

/// Scalar chunked encoding (bypasses SIMD).
fn encode_chunked_scalar(data: &[u8], dict: &Dictionary) -> String {
    let base = dict.base();
    let bits_per_char = (base as f64).log2() as usize;

    if bits_per_char == 0 || base & (base - 1) != 0 {
        // Non-power-of-2, fall back to radix
        return radix::encode(data, dict);
    }

    let mut result = String::new();
    let mut bit_buffer: u64 = 0;
    let mut bits_in_buffer = 0;

    for &byte in data {
        bit_buffer = (bit_buffer << 8) | byte as u64;
        bits_in_buffer += 8;

        while bits_in_buffer >= bits_per_char {
            bits_in_buffer -= bits_per_char;
            let index = ((bit_buffer >> bits_in_buffer) & ((1 << bits_per_char) - 1)) as usize;
            if let Some(ch) = dict.encode_digit(index) {
                result.push(ch);
            }
        }
    }

    // Handle remaining bits
    if bits_in_buffer > 0 {
        let index = ((bit_buffer << (bits_per_char - bits_in_buffer)) & ((1 << bits_per_char) - 1))
            as usize;
        if let Some(ch) = dict.encode_digit(index) {
            result.push(ch);
        }
    }

    // Add padding if needed
    if let Some(pad) = dict.padding() {
        let output_block_size = match bits_per_char {
            6 => 4, // base64
            5 => 8, // base32
            4 => 2, // base16
            _ => 1,
        };
        while !result.len().is_multiple_of(output_block_size) {
            result.push(pad);
        }
    }

    result
}

/// Scalar chunked decoding (bypasses SIMD).
fn decode_chunked_scalar(encoded: &str, dict: &Dictionary) -> Result<Vec<u8>, crate::DecodeError> {
    let base = dict.base();
    let bits_per_char = (base as f64).log2() as usize;

    if bits_per_char == 0 || base & (base - 1) != 0 {
        return radix::decode(encoded, dict);
    }

    // Strip padding
    let padding = dict.padding();
    let encoded = if let Some(pad) = padding {
        encoded.trim_end_matches(pad)
    } else {
        encoded
    };

    let mut result = Vec::new();
    let mut bit_buffer: u64 = 0;
    let mut bits_in_buffer = 0;

    for ch in encoded.chars() {
        let value = dict.decode_char(ch).ok_or(DecodeError::InvalidCharacter {
            char: ch,
            position: 0,
            input: String::new(),
            valid_chars: String::new(),
        })?;
        bit_buffer = (bit_buffer << bits_per_char) | value as u64;
        bits_in_buffer += bits_per_char;

        while bits_in_buffer >= 8 {
            bits_in_buffer -= 8;
            result.push((bit_buffer >> bits_in_buffer) as u8);
        }
    }

    Ok(result)
}

/// LUT-based SIMD encoding (uses runtime LUT construction, not hardcoded tables).
#[cfg(feature = "simd")]
fn encode_lut(data: &[u8], dict: &Dictionary) -> Option<String> {
    let base = dict.base();

    // Skip specialized paths - force LUT-based codecs only
    // 1. Try GenericSimdCodec for sequential power-of-2 dictionaries
    if let Some(codec) = simd::GenericSimdCodec::from_dictionary(dict) {
        return codec.encode(data, dict);
    }

    // 2. Try GappedSequentialCodec for near-sequential dictionaries
    if let Some(codec) = simd::GappedSequentialCodec::from_dictionary(dict) {
        return codec.encode(data, dict);
    }

    // 3. Try SmallLutCodec for small arbitrary dictionaries (≤16 chars)
    if base <= 16
        && base.is_power_of_two()
        && let Some(codec) = simd::SmallLutCodec::from_dictionary(dict)
    {
        return codec.encode(data, dict);
    }

    // 4. Try Base64LutCodec for larger arbitrary dictionaries (17-64 chars)
    if (17..=64).contains(&base)
        && base.is_power_of_two()
        && let Some(codec) = simd::Base64LutCodec::from_dictionary(dict)
    {
        return codec.encode(data, dict);
    }

    None
}

#[cfg(not(feature = "simd"))]
fn encode_lut(_data: &[u8], _dict: &Dictionary) -> Option<String> {
    None
}

/// LUT-based SIMD decoding (uses runtime LUT construction, not hardcoded tables).
#[cfg(feature = "simd")]
fn decode_lut(encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
    let base = dict.base();

    // Skip specialized paths - force LUT-based codecs only
    // 1. Try GenericSimdCodec for sequential power-of-2 dictionaries
    if let Some(codec) = simd::GenericSimdCodec::from_dictionary(dict) {
        return codec.decode(encoded, dict);
    }

    // 2. Try GappedSequentialCodec for near-sequential dictionaries
    if let Some(codec) = simd::GappedSequentialCodec::from_dictionary(dict) {
        return codec.decode(encoded, dict);
    }

    // 3. Try SmallLutCodec for small arbitrary dictionaries (≤16 chars)
    if base <= 16
        && base.is_power_of_two()
        && let Some(codec) = simd::SmallLutCodec::from_dictionary(dict)
    {
        return codec.decode(encoded, dict);
    }

    // 4. Try Base64LutCodec for larger arbitrary dictionaries (17-64 chars)
    if (17..=64).contains(&base)
        && base.is_power_of_two()
        && let Some(codec) = simd::Base64LutCodec::from_dictionary(dict)
    {
        return codec.decode(encoded, dict);
    }

    None
}

#[cfg(not(feature = "simd"))]
fn decode_lut(_encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
    None
}

/// Specialized SIMD encoding (for known RFC dictionaries).
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
fn encode_specialized(data: &[u8], dict: &Dictionary) -> Option<String> {
    use crate::simd::{
        encode_base16_simd, encode_base32_simd, encode_base64_simd, encode_base256_simd,
    };

    match dict.base() {
        16 => encode_base16_simd(data, dict),
        32 => encode_base32_simd(data, dict),
        64 => encode_base64_simd(data, dict),
        256 => encode_base256_simd(data, dict),
        _ => None,
    }
}

#[cfg(all(feature = "simd", not(target_arch = "x86_64")))]
fn encode_specialized(_data: &[u8], _dict: &Dictionary) -> Option<String> {
    // ARM doesn't have specialized paths yet (uses LUT)
    None
}

#[cfg(not(feature = "simd"))]
fn encode_specialized(_data: &[u8], _dict: &Dictionary) -> Option<String> {
    None
}

/// Specialized SIMD decoding (for known RFC dictionaries).
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
fn decode_specialized(encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
    use crate::simd::{
        decode_base16_simd, decode_base32_simd, decode_base64_simd, decode_base256_simd,
    };

    match dict.base() {
        16 => decode_base16_simd(encoded, dict),
        32 => decode_base32_simd(encoded, dict),
        64 => decode_base64_simd(encoded, dict),
        256 => decode_base256_simd(encoded, dict),
        _ => None,
    }
}

#[cfg(all(feature = "simd", not(target_arch = "x86_64")))]
fn decode_specialized(_encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
    None
}

#[cfg(not(feature = "simd"))]
fn decode_specialized(_encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DictionaryRegistry;

    fn get_test_dict(name: &str) -> Dictionary {
        let config = DictionaryRegistry::load_default().unwrap();
        let dict_config = config.get_dictionary(name).unwrap();
        let chars: Vec<char> = dict_config.effective_chars().unwrap().chars().collect();
        let padding = dict_config.padding.as_ref().and_then(|s| s.chars().next());
        let mut builder = Dictionary::builder()
            .chars(chars)
            .mode(dict_config.effective_mode());
        if let Some(p) = padding {
            builder = builder.padding(p);
        }
        builder.build().unwrap()
    }

    #[test]
    fn test_platform_detection() {
        let info = PlatformInfo::detect();
        assert!(!info.arch.is_empty());
        println!("Platform: {}", info.display());
    }

    #[test]
    fn test_path_detection_base64() {
        let dict = get_test_dict("base64");
        let paths = detect_available_paths(&dict);

        assert!(paths.contains(&EncodingPath::Scalar));
        #[cfg(feature = "simd")]
        {
            assert!(
                paths.contains(&EncodingPath::Lut) || paths.contains(&EncodingPath::Specialized)
            );
        }
    }

    #[test]
    fn test_scalar_round_trip() {
        let dict = get_test_dict("base64");
        let data = b"Hello, World!";

        let encoded = encode_with_path(data, &dict, EncodingPath::Scalar).unwrap();
        let decoded = decode_with_path(&encoded, &dict, EncodingPath::Scalar).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    fn test_paths_produce_same_output() {
        let dict = get_test_dict("base64");
        let data = b"The quick brown fox jumps over the lazy dog";
        let paths = detect_available_paths(&dict);

        let mut results: Vec<(EncodingPath, String)> = Vec::new();
        for path in &paths {
            if let Some(encoded) = encode_with_path(data, &dict, *path) {
                results.push((*path, encoded));
            }
        }

        // Compare Scalar with others, stripping padding for fair comparison
        // (LUT codecs don't add padding, specialized RFC implementations do)
        let scalar_result = results.iter().find(|(p, _)| *p == EncodingPath::Scalar);
        if let Some((_, scalar_encoded)) = scalar_result {
            let scalar_stripped = scalar_encoded.trim_end_matches('=');
            for (path, encoded) in &results {
                if *path != EncodingPath::Scalar {
                    let stripped = encoded.trim_end_matches('=');
                    assert_eq!(
                        scalar_stripped, stripped,
                        "{:?} output differs from Scalar (ignoring padding)",
                        path
                    );
                }
            }
        }
    }
}
