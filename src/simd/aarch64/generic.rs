//! Generic SIMD encoder for aarch64/NEON that works with any compatible alphabet
//!
//! This module provides a unified SIMD encoder that abstracts over
//! different alphabet structures using pluggable translation.
//!
//! Key insight: The reshuffle (bit packing) algorithms are the same
//! across alphabets of the same bit width. Only the translation layer
//! (index → character) varies.

use crate::core::dictionary::Dictionary;
use crate::simd::alphabets::{AlphabetMetadata, TranslationStrategy};
use crate::simd::translate::{SequentialTranslate, SimdTranslate};
use std::arch::aarch64::*;

/// SIMD-accelerated codec that works with any compatible alphabet
///
/// This codec uses pluggable translation to enable SIMD for sequential
/// alphabets (contiguous Unicode ranges) and known ranged patterns.
///
/// # Architecture
/// - Metadata: Analyzed alphabet structure
/// - Translator: Converts indices ↔ characters
/// - Codec: Reuses reshuffle logic from specialized implementations
pub struct GenericSimdCodec {
    metadata: AlphabetMetadata,
    translator: Box<dyn SimdTranslate>,
}

impl GenericSimdCodec {
    /// Create codec from dictionary analysis
    ///
    /// Returns None if the dictionary is not SIMD-compatible.
    pub fn from_dictionary(dict: &Dictionary) -> Option<Self> {
        let metadata = AlphabetMetadata::from_dictionary(dict);

        if !metadata.simd_compatible {
            return None;
        }

        let translator: Box<dyn SimdTranslate> = match metadata.strategy {
            TranslationStrategy::Sequential { start_codepoint } => Box::new(
                SequentialTranslate::new(start_codepoint, metadata.bits_per_symbol),
            ),
            TranslationStrategy::Ranged { .. } => {
                // For now, ranged patterns should use specialized implementations
                // Future: implement RangedTranslate
                return None;
            }
            TranslationStrategy::Arbitrary => {
                return None; // Cannot SIMD optimize
            }
        };

        Some(Self {
            metadata,
            translator,
        })
    }

    /// Encode data using SIMD acceleration
    ///
    /// Returns None if encoding fails or alphabet is incompatible.
    pub fn encode(&self, data: &[u8], dict: &Dictionary) -> Option<String> {
        // Dispatch to appropriate bit-width encoder
        match self.metadata.bits_per_symbol {
            4 => self.encode_4bit(data, dict),
            6 => self.encode_6bit(data, dict),
            8 => self.encode_8bit(data, dict),
            _ => None, // Unsupported bit width
        }
    }

    /// Decode string using SIMD acceleration
    ///
    /// Returns None if decoding fails or alphabet is incompatible.
    #[allow(dead_code)]
    pub fn decode(&self, encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
        // Dispatch to appropriate bit-width decoder
        match self.metadata.bits_per_symbol {
            4 => self.decode_4bit(encoded, dict),
            6 => self.decode_6bit(encoded, dict),
            8 => self.decode_8bit(encoded, dict),
            _ => None, // Unsupported bit width
        }
    }

    /// Encode 6-bit alphabet (base64-like)
    ///
    /// Reuses the reshuffle logic from base64.rs, replacing only the translation.
    #[cfg(target_arch = "aarch64")]
    fn encode_6bit(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        use crate::simd::aarch64::specialized::common;

        const BLOCK_SIZE: usize = 12;

        // Pre-allocate output
        let output_len = ((data.len() + 2) / 3) * 4;
        let mut result = String::with_capacity(output_len);

        unsafe {
            // Need at least 16 bytes in buffer to safely load 128 bits
            if data.len() < 16 {
                // TODO: Fall back to scalar for small inputs
                return None;
            }

            // Process blocks of 12 bytes
            let safe_len = if data.len() >= 4 { data.len() - 4 } else { 0 };
            let (num_rounds, simd_bytes) = common::calculate_blocks(safe_len, BLOCK_SIZE);

            let mut offset = 0;
            for _ in 0..num_rounds {
                // Load 16 bytes (we only use the first 12)
                let input_vec = vld1q_u8(data.as_ptr().add(offset));

                // Reshuffle bytes to extract 6-bit groups
                let reshuffled = self.reshuffle_6bit(input_vec);

                // Translate 6-bit indices to ASCII using pluggable translator
                let encoded = self.translator.translate_encode(reshuffled);

                // Store 16 output characters
                let mut output_buf = [0u8; 16];
                vst1q_u8(output_buf.as_mut_ptr(), encoded);

                // Append to result (safe because output is ASCII-like)
                for &byte in &output_buf {
                    result.push(byte as char);
                }

                offset += BLOCK_SIZE;
            }

            // TODO: Handle remainder with scalar code
            if simd_bytes < data.len() {
                // For now, we don't handle remainder
                // This will be improved in future iterations
            }
        }

        Some(result)
    }

    /// Encode 4-bit alphabet (hex-like)
    ///
    /// Reuses the nibble extraction from base16.rs, replacing only the translation.
    #[cfg(target_arch = "aarch64")]
    fn encode_4bit(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        use crate::simd::aarch64::specialized::common;

        const BLOCK_SIZE: usize = 16;

        // Pre-allocate output (2 chars per byte)
        let output_len = data.len() * 2;
        let mut result = String::with_capacity(output_len);

        unsafe {
            if data.len() < BLOCK_SIZE {
                // TODO: Fall back to scalar for small inputs
                return None;
            }

            let (num_rounds, simd_bytes) = common::calculate_blocks(data.len(), BLOCK_SIZE);

            let mut offset = 0;
            for _ in 0..num_rounds {
                // Load 16 bytes
                let input_vec = vld1q_u8(data.as_ptr().add(offset));

                // Extract high nibbles (shift right by 4)
                let hi_nibbles = vandq_u8(vshrq_n_u8(input_vec, 4), vdupq_n_u8(0x0F));

                // Extract low nibbles
                let lo_nibbles = vandq_u8(input_vec, vdupq_n_u8(0x0F));

                // Translate nibbles to ASCII using pluggable translator
                let hi_ascii = self.translator.translate_encode(hi_nibbles);
                let lo_ascii = self.translator.translate_encode(lo_nibbles);

                // Interleave high and low bytes: hi[0], lo[0], hi[1], lo[1], ...
                let result_lo = vzip1q_u8(hi_ascii, lo_ascii);
                let result_hi = vzip2q_u8(hi_ascii, lo_ascii);

                // Store 32 output characters
                let mut output_buf = [0u8; 32];
                vst1q_u8(output_buf.as_mut_ptr(), result_lo);
                vst1q_u8(output_buf.as_mut_ptr().add(16), result_hi);

                // Append to result
                for &byte in &output_buf {
                    result.push(byte as char);
                }

                offset += BLOCK_SIZE;
            }

            // TODO: Handle remainder with scalar code
            if simd_bytes < data.len() {
                // For now, we don't handle remainder
            }
        }

        Some(result)
    }

    /// Encode 8-bit alphabet (base256-like)
    ///
    /// Direct mapping with translator for sequential alphabets.
    #[cfg(target_arch = "aarch64")]
    fn encode_8bit(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        const BLOCK_SIZE: usize = 16;

        // For base256, output length equals input length
        let mut result = String::with_capacity(data.len());

        unsafe {
            if data.len() < BLOCK_SIZE {
                // TODO: Fall back to scalar for small inputs
                return None;
            }

            let num_blocks = data.len() / BLOCK_SIZE;
            let simd_bytes = num_blocks * BLOCK_SIZE;

            let mut offset = 0;
            for _ in 0..num_blocks {
                // Load 16 bytes (they are already 8-bit indices)
                let input_vec = vld1q_u8(data.as_ptr().add(offset));

                // Translate directly using pluggable translator
                let encoded = self.translator.translate_encode(input_vec);

                // Store 16 output characters
                let mut output_buf = [0u8; 16];
                vst1q_u8(output_buf.as_mut_ptr(), encoded);

                // Append to result
                for &byte in &output_buf {
                    result.push(byte as char);
                }

                offset += BLOCK_SIZE;
            }

            // TODO: Handle remainder with scalar code
            if simd_bytes < data.len() {
                // For now, we don't handle remainder
            }
        }

        Some(result)
    }

    /// Decode 4-bit alphabet (hex-like)
    ///
    /// Reverses the nibble extraction from encode_4bit.
    #[cfg(target_arch = "aarch64")]
    fn decode_4bit(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        const BLOCK_SIZE: usize = 32; // 32 chars → 16 bytes

        let encoded_bytes = encoded.as_bytes();
        let mut result = Vec::with_capacity(encoded_bytes.len() / 2);

        unsafe {
            if encoded_bytes.len() < BLOCK_SIZE {
                // TODO: Fall back to scalar for small inputs
                return None;
            }

            let num_blocks = encoded_bytes.len() / BLOCK_SIZE;
            let simd_bytes = num_blocks * BLOCK_SIZE;

            let mut offset = 0;
            for _ in 0..num_blocks {
                // Load 32 ASCII characters (16 high nibbles, 16 low nibbles interleaved)
                let input_lo = vld1q_u8(encoded_bytes.as_ptr().add(offset));
                let input_hi = vld1q_u8(encoded_bytes.as_ptr().add(offset + 16));

                // Deinterleave using uzp: extract even/odd lanes
                let (hi_chars, lo_chars) = {
                    // Combine into even/odd extraction
                    let even_lo = vuzp1q_u8(input_lo, input_hi); // Even positions: 0,2,4,...
                    let odd_lo = vuzp2q_u8(input_lo, input_hi); // Odd positions: 1,3,5,...
                    (even_lo, odd_lo)
                };

                // Translate chars to nibble values
                let hi_vals = self.translator.translate_decode(hi_chars)?;
                let lo_vals = self.translator.translate_decode(lo_chars)?;

                // Pack nibbles into bytes: (high << 4) | low
                let bytes = vorrq_u8(vshlq_n_u8(hi_vals, 4), lo_vals);

                // Store 16 output bytes
                let mut output_buf = [0u8; 16];
                vst1q_u8(output_buf.as_mut_ptr(), bytes);
                result.extend_from_slice(&output_buf);

                offset += BLOCK_SIZE;
            }

            // TODO: Handle remainder with scalar
            if simd_bytes < encoded_bytes.len() {
                // For now, we don't handle remainder
            }
        }

        Some(result)
    }

    /// Decode 6-bit alphabet (base64-like)
    ///
    /// Uses the same maddubs/madd trick as specialized base64 decode.
    #[cfg(target_arch = "aarch64")]
    fn decode_6bit(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        const BLOCK_SIZE: usize = 16; // 16 chars → 12 bytes

        let encoded_bytes = encoded.as_bytes();
        let mut result = Vec::with_capacity(encoded_bytes.len() * 3 / 4);

        unsafe {
            if encoded_bytes.len() < BLOCK_SIZE {
                // TODO: Fall back to scalar for small inputs
                return None;
            }

            let num_blocks = encoded_bytes.len() / BLOCK_SIZE;
            let simd_bytes = num_blocks * BLOCK_SIZE;

            for round in 0..num_blocks {
                let offset = round * BLOCK_SIZE;

                // Load 16 ASCII chars
                let chars = vld1q_u8(encoded_bytes.as_ptr().add(offset));

                // Translate to 6-bit indices (validation included)
                let indices = self.translator.translate_decode(chars)?;

                // Unpack 6-bit indices back to bytes (inverse of reshuffle_6bit)
                let bytes = self.unshuffle_6bit(indices);

                // Store 12 output bytes
                let mut output_buf = [0u8; 16];
                vst1q_u8(output_buf.as_mut_ptr(), bytes);
                result.extend_from_slice(&output_buf[..12]);
            }

            // TODO: Handle remainder with scalar
            if simd_bytes < encoded_bytes.len() {
                // For now, we don't handle remainder
            }
        }

        Some(result)
    }

    /// Decode 8-bit alphabet (base256-like)
    ///
    /// Direct translation, no bit unpacking needed.
    #[cfg(target_arch = "aarch64")]
    fn decode_8bit(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        const BLOCK_SIZE: usize = 16; // 16 chars → 16 bytes

        let encoded_bytes = encoded.as_bytes();
        let mut result = Vec::with_capacity(encoded_bytes.len());

        unsafe {
            if encoded_bytes.len() < BLOCK_SIZE {
                // TODO: Fall back to scalar for small inputs
                return None;
            }

            let num_blocks = encoded_bytes.len() / BLOCK_SIZE;
            let simd_bytes = num_blocks * BLOCK_SIZE;

            let mut offset = 0;
            for _ in 0..num_blocks {
                // Load 16 ASCII chars
                let chars = vld1q_u8(encoded_bytes.as_ptr().add(offset));

                // Translate to bytes (validation included)
                let bytes = self.translator.translate_decode(chars)?;

                // No unpacking needed - direct 1:1 mapping
                let mut output_buf = [0u8; 16];
                vst1q_u8(output_buf.as_mut_ptr(), bytes);
                result.extend_from_slice(&output_buf);

                offset += BLOCK_SIZE;
            }

            // TODO: Handle remainder with scalar
            if simd_bytes < encoded_bytes.len() {
                // For now, we don't handle remainder
            }
        }

        Some(result)
    }

    /// Reshuffle bytes and extract 6-bit indices from 12 input bytes
    ///
    /// This is the same algorithm as base64.rs::reshuffle() ported to NEON
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn reshuffle_6bit(&self, input: uint8x16_t) -> uint8x16_t {
        // Input, bytes MSB to LSB (little endian):
        // 0 0 0 0 l k j i h g f e d c b a
        //
        // Each group of 3 input bytes (24 bits) becomes 4 output bytes (4 x 6 bits)

        // Shuffle indices to duplicate bytes for 6-bit extraction
        // Each group of 3 input bytes becomes 4 output bytes with duplicates
        let shuffle_indices = vld1q_u8(
            [
                0, 0, 1, 2, // bytes 0-2 -> positions 0-3
                3, 3, 4, 5, // bytes 3-5 -> positions 4-7
                6, 6, 7, 8, // bytes 6-8 -> positions 8-11
                9, 9, 10, 11, // bytes 9-11 -> positions 12-15
            ]
            .as_ptr(),
        );

        let shuffled = vqtbl1q_u8(input, shuffle_indices);

        // Extract 6-bit groups using multiplication tricks
        // For 3 bytes ABC (24 bits) -> 4 groups of 6 bits

        let shuffled_u32 = vreinterpretq_u32_u8(shuffled);

        // First extraction: get bits for positions 0 and 2 in each group of 4
        let t0 = vandq_u32(shuffled_u32, vdupq_n_u32(0x0FC0FC00_u32));
        let t1 = {
            let t0_u16 = vreinterpretq_u16_u32(t0);
            let mult = vmulq_n_u16(t0_u16, 0x0040);
            vreinterpretq_u32_u16(vshrq_n_u16(mult, 10))
        };

        // Second extraction: get bits for positions 1 and 3 in each group of 4
        let t2 = vandq_u32(shuffled_u32, vdupq_n_u32(0x003F03F0_u32));
        let t3 = {
            let t2_u16 = vreinterpretq_u16_u32(t2);
            let mult = vmulq_n_u16(t2_u16, 0x0010);
            vreinterpretq_u32_u16(vshrq_n_u16(mult, 6))
        };

        // Combine the two results
        vreinterpretq_u8_u32(vorrq_u32(t1, t3))
    }

    /// Unshuffle 6-bit indices back to 8-bit bytes
    ///
    /// Inverse of reshuffle_6bit - converts 16 bytes of 6-bit indices to 12 bytes of data
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn unshuffle_6bit(&self, indices: uint8x16_t) -> uint8x16_t {
        // This is the same algorithm as base64.rs::reshuffle_decode
        // Uses multiply-add to efficiently pack 6-bit values back to 8-bit

        // Stage 1: Merge adjacent pairs using multiply-add
        // Simulate maddubs: multiply unsigned bytes and add adjacent pairs
        let pairs = vreinterpretq_u16_u8(indices);
        let even = vandq_u16(pairs, vdupq_n_u16(0xFF));
        let odd = vshrq_n_u16(pairs, 8);
        let merge_result = vaddq_u16(even, vshlq_n_u16(odd, 6));

        // Stage 2: Combine 16-bit pairs into 32-bit values
        // Simulate madd: multiply 16-bit values and add adjacent pairs
        let merge_u32 = vreinterpretq_u32_u16(merge_result);
        let lo = vandq_u32(merge_u32, vdupq_n_u32(0xFFFF));
        let hi = vshrq_n_u32(merge_u32, 16);
        let final_32bit = vorrq_u32(vshlq_n_u32(lo, 12), hi);

        // Stage 3: Extract the valid bytes from each 32-bit group
        // Each group of 4 indices (24 bits) became 1 32-bit value
        // We extract the 3 meaningful bytes from each 32-bit group
        let shuffle_mask = vld1q_u8(
            [
                2, 1, 0, // first group of 3 bytes (reversed for little endian)
                6, 5, 4, // second group of 3 bytes
                10, 9, 8, // third group of 3 bytes
                14, 13, 12, // fourth group of 3 bytes
                255, 255, 255, 255, // unused bytes (will be zero)
            ]
            .as_ptr(),
        );

        let result_bytes = vreinterpretq_u8_u32(final_32bit);
        vqtbl1q_u8(result_bytes, shuffle_mask)
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn encode_6bit(&self, _data: &[u8], _dict: &Dictionary) -> Option<String> {
        None
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn encode_4bit(&self, _data: &[u8], _dict: &Dictionary) -> Option<String> {
        None
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn encode_8bit(&self, _data: &[u8], _dict: &Dictionary) -> Option<String> {
        None
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn decode_6bit(&self, _encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        None
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn decode_4bit(&self, _encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        None
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn decode_8bit(&self, _encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::EncodingMode;

    #[test]
    fn test_sequential_base64_creation() {
        // Create a sequential base64 alphabet starting at Latin Extended-A (U+0100)
        let chars: Vec<char> = (0x100..0x140)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict);
        assert!(codec.is_some(), "Should create codec for sequential base64");

        let codec = codec.unwrap();
        assert_eq!(codec.metadata.bits_per_symbol, 6);
        assert!(matches!(
            codec.metadata.strategy,
            TranslationStrategy::Sequential {
                start_codepoint: 0x100
            }
        ));
    }

    #[test]
    fn test_sequential_hex_creation() {
        // Create a sequential hex alphabet starting at '!' (U+0021)
        let chars: Vec<char> = (0x21..0x31).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict);
        assert!(codec.is_some(), "Should create codec for sequential hex");

        let codec = codec.unwrap();
        assert_eq!(codec.metadata.bits_per_symbol, 4);
        assert!(matches!(
            codec.metadata.strategy,
            TranslationStrategy::Sequential {
                start_codepoint: 0x21
            }
        ));
    }

    #[test]
    fn test_sequential_base256_creation() {
        // Create a sequential base256 alphabet using Latin Extended-A range
        let chars: Vec<char> = (0x100..0x200)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict);
        assert!(
            codec.is_some(),
            "Should create codec for sequential base256"
        );

        let codec = codec.unwrap();
        assert_eq!(codec.metadata.bits_per_symbol, 8);
        assert!(matches!(
            codec.metadata.strategy,
            TranslationStrategy::Sequential {
                start_codepoint: 0x100
            }
        ));
    }

    #[test]
    fn test_arbitrary_alphabet_rejected() {
        // Create an arbitrary (shuffled) alphabet
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBAzyxwvutsrqponmlkjihgfedcba9876543210+/"
            .chars()
            .collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict);
        assert!(codec.is_none(), "Should reject arbitrary alphabet");
    }

    #[test]
    fn test_non_power_of_two_rejected() {
        // Create base10 (not power of 2)
        let chars: Vec<char> = "0123456789".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict);
        assert!(codec.is_none(), "Should reject non-power-of-2 base");
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_encode_4bit_sequential() {
        // Create sequential hex starting at '0' (U+0030)
        let chars: Vec<char> = (0x30..0x40).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict).unwrap();

        // Test data: 16 bytes
        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10";
        let result = codec.encode_4bit(data, &dict);

        assert!(result.is_some());
        let encoded = result.unwrap();

        // Verify length: 16 bytes -> 32 hex chars
        assert_eq!(encoded.len(), 32);

        // Verify first few characters
        // 0x01 -> '0', '1'
        // 0x23 -> '2', '3'
        assert!(encoded.starts_with("0123"));
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_encode_6bit_sequential() {
        // Create sequential base64 starting at '!' (U+0021) - ASCII range
        // This avoids multi-byte UTF-8 encoding issues in the test
        // Uses 64 printable ASCII chars: '!' through '`' (0x21..0x61)
        let chars: Vec<char> = (0x21..0x61).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict).unwrap();

        // Test data: 16 bytes (will process 12 bytes in SIMD, ignore remainder)
        let data = b"Hello, World!!!!";
        let result = codec.encode_6bit(data, &dict);

        assert!(result.is_some());
        let encoded = result.unwrap();

        // Verify length: 12 bytes processed -> 16 base64 chars (all ASCII)
        assert_eq!(encoded.len(), 16);

        // Verify all output is ASCII (< 0x80)
        for byte in encoded.as_bytes() {
            assert!(*byte < 0x80, "Output should be ASCII, got 0x{:02X}", byte);
        }
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_custom_alphabet_integration() {
        // Demonstrate that a user-defined sequential alphabet gets SIMD acceleration
        // Note: Currently only works for ASCII-range alphabets (< 0x80)
        // TODO: Add UTF-8 encoding support for higher Unicode ranges

        // Create custom base16 alphabet starting at ASCII '!' (0x21)
        // This gives us '!' through '0' (0x21..0x31)
        let chars: Vec<char> = (0x21..0x31).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        // Verify it's detected as SIMD-compatible
        let metadata = AlphabetMetadata::from_dictionary(&dict);
        assert!(
            metadata.simd_compatible,
            "Custom alphabet should be SIMD-compatible"
        );
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Sequential {
                start_codepoint: 0x21
            }
        ));

        // Create codec
        let codec = GenericSimdCodec::from_dictionary(&dict)
            .expect("Should create codec for custom alphabet");

        // Encode data: 16 bytes
        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10";
        let result = codec.encode_4bit(data, &dict);

        assert!(result.is_some(), "Should encode with custom alphabet");
        let encoded = result.unwrap();

        // Verify output length: 16 bytes -> 32 hex chars
        assert_eq!(encoded.len(), 32, "16 bytes should produce 32 hex chars");

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
    #[cfg(target_arch = "aarch64")]
    fn test_decode_4bit_round_trip() {
        // Create sequential hex starting at '0' (U+0030)
        let chars: Vec<char> = (0x30..0x40).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict).unwrap();

        // Test data: 16 bytes
        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10";

        // Encode
        let encoded = codec.encode(data, &dict).expect("Encode failed");

        // Decode
        let decoded = codec.decode(&encoded, &dict).expect("Decode failed");

        // Verify
        assert_eq!(&decoded[..], &data[..], "Round-trip failed");
    }
}
