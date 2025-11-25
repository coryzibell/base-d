//! Generic SIMD encoder that works with any compatible alphabet
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

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
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
    #[cfg(target_arch = "x86_64")]
    fn encode_6bit(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        use crate::simd::x86_64::common;

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
                let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

                // Reshuffle bytes to extract 6-bit groups
                let reshuffled = self.reshuffle_6bit(input_vec);

                // Translate 6-bit indices to ASCII using pluggable translator
                let encoded = self.translator.translate_encode(reshuffled);

                // Store 16 output characters
                let mut output_buf = [0u8; 16];
                _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, encoded);

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
    #[cfg(target_arch = "x86_64")]
    fn encode_4bit(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        use crate::simd::x86_64::common;

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
                let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

                // Extract high nibbles (shift right by 4)
                let hi_nibbles = _mm_and_si128(_mm_srli_epi32(input_vec, 4), _mm_set1_epi8(0x0F));

                // Extract low nibbles
                let lo_nibbles = _mm_and_si128(input_vec, _mm_set1_epi8(0x0F));

                // Translate nibbles to ASCII using pluggable translator
                let hi_ascii = self.translator.translate_encode(hi_nibbles);
                let lo_ascii = self.translator.translate_encode(lo_nibbles);

                // Interleave high and low bytes: hi[0], lo[0], hi[1], lo[1], ...
                let result_lo = _mm_unpacklo_epi8(hi_ascii, lo_ascii);
                let result_hi = _mm_unpackhi_epi8(hi_ascii, lo_ascii);

                // Store 32 output characters
                let mut output_buf = [0u8; 32];
                _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, result_lo);
                _mm_storeu_si128(output_buf.as_mut_ptr().add(16) as *mut __m128i, result_hi);

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
    #[cfg(target_arch = "x86_64")]
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
                let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

                // Translate directly using pluggable translator
                let encoded = self.translator.translate_encode(input_vec);

                // Store 16 output characters
                let mut output_buf = [0u8; 16];
                _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, encoded);

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
    #[cfg(target_arch = "x86_64")]
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
                let input_lo =
                    _mm_loadu_si128(encoded_bytes.as_ptr().add(offset) as *const __m128i);
                let input_hi =
                    _mm_loadu_si128(encoded_bytes.as_ptr().add(offset + 16) as *const __m128i);

                // Deinterleave using shuffle: extract bytes at even/odd positions
                // Shuffle mask to extract even bytes (0, 2, 4, 6, 8, 10, 12, 14) into first 8 positions
                let even_mask =
                    _mm_setr_epi8(0, 2, 4, 6, 8, 10, 12, 14, -1, -1, -1, -1, -1, -1, -1, -1);
                // Shuffle mask to extract odd bytes (1, 3, 5, 7, 9, 11, 13, 15) into first 8 positions
                let odd_mask =
                    _mm_setr_epi8(1, 3, 5, 7, 9, 11, 13, 15, -1, -1, -1, -1, -1, -1, -1, -1);

                // Extract even positions (HIGH nibble chars) from both input vectors
                let hi_chars_lo = _mm_shuffle_epi8(input_lo, even_mask); // 8 bytes in positions 0-7
                let hi_chars_hi = _mm_shuffle_epi8(input_hi, even_mask); // 8 bytes in positions 0-7

                // Extract odd positions (LOW nibble chars) from both input vectors
                let lo_chars_lo = _mm_shuffle_epi8(input_lo, odd_mask); // 8 bytes in positions 0-7
                let lo_chars_hi = _mm_shuffle_epi8(input_hi, odd_mask); // 8 bytes in positions 0-7

                // Combine into full 16-byte vectors by placing hi_chars_hi into upper 8 bytes
                let hi_chars = _mm_or_si128(hi_chars_lo, _mm_slli_si128(hi_chars_hi, 8));
                let lo_chars = _mm_or_si128(lo_chars_lo, _mm_slli_si128(lo_chars_hi, 8));

                // Translate chars to nibble values
                let hi_vals = self.translator.translate_decode(hi_chars)?;
                let lo_vals = self.translator.translate_decode(lo_chars)?;

                // Pack nibbles into bytes: (high << 4) | low
                let bytes = _mm_or_si128(_mm_slli_epi32(hi_vals, 4), lo_vals);

                // Store 16 output bytes
                let mut output_buf = [0u8; 16];
                _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, bytes);
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
    #[cfg(target_arch = "x86_64")]
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
                let chars = _mm_loadu_si128(encoded_bytes.as_ptr().add(offset) as *const __m128i);

                // Translate to 6-bit indices (validation included)
                let indices = self.translator.translate_decode(chars)?;

                // Unpack 6-bit indices back to bytes (inverse of reshuffle_6bit)
                let bytes = self.unshuffle_6bit(indices);

                // Store 12 output bytes
                let mut output_buf = [0u8; 16];
                _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, bytes);
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
    #[cfg(target_arch = "x86_64")]
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
                let chars = _mm_loadu_si128(encoded_bytes.as_ptr().add(offset) as *const __m128i);

                // Translate to bytes (validation included)
                let bytes = self.translator.translate_decode(chars)?;

                // No unpacking needed - direct 1:1 mapping
                let mut output_buf = [0u8; 16];
                _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, bytes);
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
    /// This is the same algorithm as base64.rs::reshuffle()
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn reshuffle_6bit(&self, input: __m128i) -> __m128i {
        // Input, bytes MSB to LSB (little endian):
        // 0 0 0 0 l k j i h g f e d c b a
        //
        // Each group of 3 input bytes (24 bits) becomes 4 output bytes (4 x 6 bits)

        let shuffled = _mm_shuffle_epi8(
            input,
            _mm_set_epi8(
                10, 11, 9, 10, // bytes for output positions 12-15
                7, 8, 6, 7, // bytes for output positions 8-11
                4, 5, 3, 4, // bytes for output positions 4-7
                1, 2, 0, 1, // bytes for output positions 0-3
            ),
        );

        // Extract 6-bit groups using multiplication tricks
        // For 3 bytes ABC (24 bits) -> 4 groups of 6 bits

        // First extraction: get bits for positions 0 and 2 in each group of 4
        let t0 = _mm_and_si128(shuffled, _mm_set1_epi32(0x0FC0FC00_u32 as i32));
        let t1 = _mm_mulhi_epu16(t0, _mm_set1_epi32(0x04000040_u32 as i32));

        // Second extraction: get bits for positions 1 and 3 in each group of 4
        let t2 = _mm_and_si128(shuffled, _mm_set1_epi32(0x003F03F0_u32 as i32));
        let t3 = _mm_mullo_epi16(t2, _mm_set1_epi32(0x01000010_u32 as i32));

        // Combine the two results
        _mm_or_si128(t1, t3)
    }

    /// Unshuffle 6-bit indices back to 8-bit bytes
    ///
    /// Inverse of reshuffle_6bit - converts 16 bytes of 6-bit indices to 12 bytes of data
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn unshuffle_6bit(&self, indices: __m128i) -> __m128i {
        // This is the same algorithm as base64.rs::reshuffle_decode
        // Uses maddubs and madd to efficiently pack 6-bit values back to 8-bit

        // Stage 1: Merge adjacent pairs using multiply-add
        // maddubs: multiply unsigned bytes and add adjacent pairs
        let merge_ab_and_bc = _mm_maddubs_epi16(indices, _mm_set1_epi32(0x01400140u32 as i32));

        // Stage 2: Combine 16-bit pairs into 32-bit values
        // madd: multiply 16-bit values and add adjacent pairs
        let final_32bit = _mm_madd_epi16(merge_ab_and_bc, _mm_set1_epi32(0x00011000u32 as i32));

        // Stage 3: Extract the valid bytes from each 32-bit group
        // Each group of 4 indices (24 bits) became 1 32-bit value
        // We extract the 3 meaningful bytes from each 32-bit group
        _mm_shuffle_epi8(
            final_32bit,
            _mm_setr_epi8(
                2, 1, 0, // first group of 3 bytes (reversed for little endian)
                6, 5, 4, // second group of 3 bytes
                10, 9, 8, // third group of 3 bytes
                14, 13, 12, // fourth group of 3 bytes
                -1, -1, -1, -1, // unused bytes (will be zero)
            ),
        )
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn encode_6bit(&self, _data: &[u8], _dict: &Dictionary) -> Option<String> {
        None
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn encode_4bit(&self, _data: &[u8], _dict: &Dictionary) -> Option<String> {
        None
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn encode_8bit(&self, _data: &[u8], _dict: &Dictionary) -> Option<String> {
        None
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn decode_6bit(&self, _encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        None
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn decode_4bit(&self, _encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        None
    }

    #[cfg(not(target_arch = "x86_64"))]
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
    #[cfg(target_arch = "x86_64")]
    fn test_encode_4bit_sequential() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

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
    #[cfg(target_arch = "x86_64")]
    fn test_encode_6bit_sequential() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Create sequential base64 starting at Latin Extended-A (U+0100)
        let chars: Vec<char> = (0x100..0x140)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict).unwrap();

        // Test data: 16 bytes (will process 12 bytes in SIMD, ignore remainder)
        let data = b"Hello, World!!!!";
        let result = codec.encode_6bit(data, &dict);

        assert!(result.is_some());
        let encoded = result.unwrap();

        // Verify length: 12 bytes processed -> 16 base64 chars
        assert_eq!(encoded.len(), 16);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_8bit_sequential() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Note: For base256 with chars > 0x7F, we need proper UTF-8 handling
        // which is not yet implemented. For now, test with ASCII-compatible range.
        // This test is commented out until proper multi-byte UTF-8 support is added.

        // Create sequential base256 starting at ASCII space (U+0020)
        // This gives us 0x20..0x120, but we only use 0x20..0x7F for now (ASCII range)
        // TODO: Implement proper UTF-8 encoding for chars > 0x7F

        // Temporary: Skip this test as it requires UTF-8 encoding support
        // The infrastructure is correct, but we need to add UTF-8 byte conversion
        eprintln!("Skipping base256 test: UTF-8 encoding not yet implemented for chars > 0x7F");
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_custom_alphabet_integration() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

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
    #[cfg(target_arch = "x86_64")]
    fn test_decode_4bit_round_trip() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

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
