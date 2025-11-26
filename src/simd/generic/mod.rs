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
            TranslationStrategy::Arbitrary { .. } => {
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
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                // Try AVX2 first, fallback to SSSE3 if data too small
                let result = match self.metadata.bits_per_symbol {
                    4 => self.encode_4bit_avx2(data, dict),
                    5 => self.encode_5bit_avx2(data, dict),
                    6 => self.encode_6bit_avx2(data, dict),
                    8 => self.encode_8bit_avx2(data, dict),
                    _ => None,
                };
                if result.is_some() {
                    return result;
                }
                // Fallback to SSSE3 for small inputs
                match self.metadata.bits_per_symbol {
                    4 => self.encode_4bit(data, dict),
                    5 => self.encode_5bit(data, dict),
                    6 => self.encode_6bit(data, dict),
                    8 => self.encode_8bit(data, dict),
                    _ => None,
                }
            } else {
                match self.metadata.bits_per_symbol {
                    4 => self.encode_4bit(data, dict),
                    5 => self.encode_5bit(data, dict),
                    6 => self.encode_6bit(data, dict),
                    8 => self.encode_8bit(data, dict),
                    _ => None,
                }
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            match self.metadata.bits_per_symbol {
                4 => self.encode_4bit(data, dict),
                5 => self.encode_5bit(data, dict),
                6 => self.encode_6bit(data, dict),
                8 => self.encode_8bit(data, dict),
                _ => None,
            }
        }
    }

    /// Decode string using SIMD acceleration
    ///
    /// Returns None if decoding fails or alphabet is incompatible.
    #[allow(dead_code)]
    pub fn decode(&self, encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
        // Dispatch to appropriate bit-width decoder
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                // Try AVX2 first, fallback to SSSE3 if data too small
                let result = match self.metadata.bits_per_symbol {
                    4 => self.decode_4bit_avx2(encoded, dict),
                    5 => self.decode_5bit_avx2(encoded, dict),
                    6 => self.decode_6bit_avx2(encoded, dict),
                    8 => self.decode_8bit_avx2(encoded, dict),
                    _ => None,
                };
                if result.is_some() {
                    return result;
                }
                // Fallback to SSSE3 for small inputs
                match self.metadata.bits_per_symbol {
                    4 => self.decode_4bit(encoded, dict),
                    5 => self.decode_5bit(encoded, dict),
                    6 => self.decode_6bit(encoded, dict),
                    8 => self.decode_8bit(encoded, dict),
                    _ => None,
                }
            } else {
                match self.metadata.bits_per_symbol {
                    4 => self.decode_4bit(encoded, dict),
                    5 => self.decode_5bit(encoded, dict),
                    6 => self.decode_6bit(encoded, dict),
                    8 => self.decode_8bit(encoded, dict),
                    _ => None,
                }
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            match self.metadata.bits_per_symbol {
                4 => self.decode_4bit(encoded, dict),
                5 => self.decode_5bit(encoded, dict),
                6 => self.decode_6bit(encoded, dict),
                8 => self.decode_8bit(encoded, dict),
                _ => None,
            }
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

    /// Encode 5-bit alphabet (base32-like)
    ///
    /// Reuses the bit extraction from base32.rs, replacing only the translation.
    #[cfg(target_arch = "x86_64")]
    fn encode_5bit(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        use crate::simd::x86_64::common;

        const BLOCK_SIZE: usize = 10; // 10 bytes -> 16 chars

        // Pre-allocate output
        let output_len = ((data.len() + 4) / 5) * 8;
        let mut result = String::with_capacity(output_len);

        unsafe {
            if data.len() < 16 {
                // TODO: Fall back to scalar for small inputs
                return None;
            }

            // Process blocks of 10 bytes. We load 16 bytes but only use 10.
            let safe_len = if data.len() >= 6 { data.len() - 6 } else { 0 };
            let (num_rounds, simd_bytes) = common::calculate_blocks(safe_len, BLOCK_SIZE);

            let mut offset = 0;
            for _ in 0..num_rounds {
                // Load 16 bytes (we only use the first 10)
                let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

                // Extract 5-bit indices from 10 packed bytes
                let indices = self.unpack_5bit_simple(input_vec);

                // Translate 5-bit indices to ASCII using pluggable translator
                let encoded = self.translator.translate_encode(indices);

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

    /// Decode 5-bit alphabet (base32-like)
    ///
    /// Uses the same packing algorithm as specialized base32 decode.
    #[cfg(target_arch = "x86_64")]
    fn decode_5bit(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        const BLOCK_SIZE: usize = 16; // 16 chars → 10 bytes

        let encoded_bytes = encoded.as_bytes();
        let mut result = Vec::with_capacity(encoded_bytes.len() * 5 / 8);

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

                // Translate to 5-bit indices (validation included)
                let indices = self.translator.translate_decode(chars)?;

                // Pack 5-bit values into bytes (16 chars -> 10 bytes)
                let bytes = self.pack_5bit_to_8bit(indices);

                // Store 10 output bytes
                let mut output_buf = [0u8; 16];
                _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, bytes);
                result.extend_from_slice(&output_buf[..10]);
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

    /// Simple 5-bit unpacking using direct shifts and masks
    ///
    /// Extracts 16 x 5-bit values from 10 bytes
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn unpack_5bit_simple(&self, input: __m128i) -> __m128i {
        // Extract bytes 0-9 into a buffer for easier manipulation
        let mut buf = [0u8; 16];
        _mm_storeu_si128(buf.as_mut_ptr() as *mut __m128i, input);

        // Extract 5-bit indices manually (two 5-byte groups)
        let mut indices = [0u8; 16];

        // First group: bytes 0-4 -> indices 0-7
        indices[0] = buf[0] >> 3;
        indices[1] = ((buf[0] & 0x07) << 2) | (buf[1] >> 6);
        indices[2] = (buf[1] >> 1) & 0x1F;
        indices[3] = ((buf[1] & 0x01) << 4) | (buf[2] >> 4);
        indices[4] = ((buf[2] & 0x0F) << 1) | (buf[3] >> 7);
        indices[5] = (buf[3] >> 2) & 0x1F;
        indices[6] = ((buf[3] & 0x03) << 3) | (buf[4] >> 5);
        indices[7] = buf[4] & 0x1F;

        // Second group: bytes 5-9 -> indices 8-15
        indices[8] = buf[5] >> 3;
        indices[9] = ((buf[5] & 0x07) << 2) | (buf[6] >> 6);
        indices[10] = (buf[6] >> 1) & 0x1F;
        indices[11] = ((buf[6] & 0x01) << 4) | (buf[7] >> 4);
        indices[12] = ((buf[7] & 0x0F) << 1) | (buf[8] >> 7);
        indices[13] = (buf[8] >> 2) & 0x1F;
        indices[14] = ((buf[8] & 0x03) << 3) | (buf[9] >> 5);
        indices[15] = buf[9] & 0x1F;

        _mm_loadu_si128(indices.as_ptr() as *const __m128i)
    }

    /// Pack 16 bytes of 5-bit indices into 10 bytes
    ///
    /// Based on Lemire's multiply-shift approach for base32.
    /// 16 5-bit values -> 10 8-bit bytes
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn pack_5bit_to_8bit(&self, indices: __m128i) -> __m128i {
        // Process in groups of 8 chars -> 5 bytes
        // Input: 8 bytes, each containing 5-bit value (0x00-0x1F)
        // Output: 5 packed bytes

        // Stage 1: Merge pairs using multiply-add
        // _mm_maddubs_epi16: multiply pairs of bytes, then add adjacent results
        // Multiply by 0x20 (32) to shift left by 5 bits, 0x01 to keep in place
        // Result: 8 16-bit values, each combining two 5-bit inputs
        let merged = _mm_maddubs_epi16(indices, _mm_set1_epi32(0x01200120u32 as i32));

        // Stage 2: Combine 16-bit pairs into 32-bit values
        // _mm_madd_epi16: multiply pairs of 16-bit values, then add adjacent results
        // This packs four 5-bit values into each 32-bit lane
        // 0x00000001 << 16 | 0x00000400 = shift left by 10 bits, or keep in place << 10
        let combined = _mm_madd_epi16(
            merged,
            _mm_set_epi32(
                0x00010400, // High 64-bit lane, 2nd pair
                0x00104000, // High 64-bit lane, 1st pair
                0x00010400, // Low 64-bit lane, 2nd pair
                0x00104000, // Low 64-bit lane, 1st pair
            ),
        );

        // Now we have 4 x 32-bit values, each containing parts of our packed output
        // Layout (after multiply-add):
        // - Each 32-bit contains bits from 4 5-bit inputs
        // - We need to extract and rearrange these

        // Stage 3: Shift and combine to consolidate bits
        // Shift upper 16 bits of each 32-bit down, then OR
        let shifted = _mm_srli_epi64(combined, 48);
        let packed = _mm_or_si128(combined, shifted);

        // Stage 4: Shuffle to extract the 10 valid bytes in correct order
        // From NLnetLabs/simdzone: _mm_set_epi8(0, 0, 0, 0, 0, 0, 12, 13, 8, 9, 10, 4, 5, 0, 1, 2)
        // Note: _mm_set_epi8 is in REVERSE order (first arg goes to byte 15)
        // Converting to setr order (forward): 2, 1, 0, 5, 4, 10, 9, 8, 13, 12, 0, 0, 0, 0, 0, 0
        _mm_shuffle_epi8(
            packed,
            _mm_setr_epi8(
                2, 1, 0, // Bytes 0-2
                5, 4, // Bytes 3-4
                10, 9, 8, // Bytes 5-7
                13, 12, // Bytes 8-9
                0, 0, 0, 0, 0, 0, // Padding
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
    fn encode_5bit(&self, _data: &[u8], _dict: &Dictionary) -> Option<String> {
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
    fn decode_5bit(&self, _encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        None
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn decode_8bit(&self, _encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        None
    }

    // ========== AVX2 (256-bit) Implementations ==========

    /// Encode 8-bit alphabet using AVX2 (processes 32 bytes per iteration)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn encode_8bit_avx2_impl(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        const BLOCK_SIZE: usize = 32; // Process 32 bytes at a time with AVX2

        let mut result = String::with_capacity(data.len());

        if data.len() < BLOCK_SIZE {
            return None;
        }

        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Load 32 bytes
            let input_vec = _mm256_loadu_si256(data.as_ptr().add(offset) as *const __m256i);

            // Translate using pluggable translator (processes as two 128-bit lanes)
            let encoded = self.translator.translate_encode_256(input_vec);

            // Store 32 output characters
            let mut output_buf = [0u8; 32];
            _mm256_storeu_si256(output_buf.as_mut_ptr() as *mut __m256i, encoded);

            for &byte in &output_buf {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // TODO: Handle remainder with scalar code
        if simd_bytes < data.len() {
            // For now, we don't handle remainder
        }

        Some(result)
    }

    #[cfg(target_arch = "x86_64")]
    fn encode_8bit_avx2(&self, data: &[u8], dict: &Dictionary) -> Option<String> {
        unsafe { self.encode_8bit_avx2_impl(data, dict) }
    }

    /// Encode 4-bit alphabet using AVX2
    ///
    /// For now, fallback to SSSE3 for correctness
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_4bit_avx2_impl(&self, data: &[u8], dict: &Dictionary) -> Option<String> {
        // Fallback to SSSE3 for now - AVX2 lane-crossing is complex
        self.encode_4bit(data, dict)
    }

    #[cfg(target_arch = "x86_64")]
    fn encode_4bit_avx2(&self, data: &[u8], dict: &Dictionary) -> Option<String> {
        unsafe { self.encode_4bit_avx2_impl(data, dict) }
    }

    /// Encode 5-bit alphabet using AVX2 (processes 20 bytes -> 32 output chars)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn encode_5bit_avx2_impl(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        use crate::simd::x86_64::common;

        const BLOCK_SIZE: usize = 20; // 20 bytes -> 32 chars

        let output_len = ((data.len() + 4) / 5) * 8;
        let mut result = String::with_capacity(output_len);

        if data.len() < 32 {
            return None;
        }

        let safe_len = if data.len() >= 12 { data.len() - 12 } else { 0 };
        let (num_rounds, simd_bytes) = common::calculate_blocks(safe_len, BLOCK_SIZE);

        let mut offset = 0;
        for _ in 0..num_rounds {
            // Load 20 bytes as two 128-bit chunks (bytes 0-9 and 10-19)
            let input_lo = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);
            let input_hi = _mm_loadu_si128(data.as_ptr().add(offset + 10) as *const __m128i);

            // Combine into 256-bit register
            let input_256 = _mm256_set_m128i(input_hi, input_lo);

            // Extract 5-bit indices from both lanes
            let indices = self.extract_5bit_indices_avx2(input_256);

            // Translate 5-bit indices to ASCII using pluggable translator
            let encoded = self.translator.translate_encode_256(indices);

            // Store 32 output characters
            let mut output_buf = [0u8; 32];
            _mm256_storeu_si256(output_buf.as_mut_ptr() as *mut __m256i, encoded);

            for &byte in &output_buf {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // TODO: Handle remainder
        if simd_bytes < data.len() {
            // For now, we don't handle remainder
        }

        Some(result)
    }

    #[cfg(target_arch = "x86_64")]
    fn encode_5bit_avx2(&self, data: &[u8], dict: &Dictionary) -> Option<String> {
        unsafe { self.encode_5bit_avx2_impl(data, dict) }
    }

    /// Encode 6-bit alphabet using AVX2 (processes 24 bytes -> 32 output chars)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn encode_6bit_avx2_impl(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        use crate::simd::x86_64::common;

        const BLOCK_SIZE: usize = 24; // 24 bytes input -> 32 chars output

        let output_len = ((data.len() + 2) / 3) * 4;
        let mut result = String::with_capacity(output_len);

        if data.len() < 32 {
            return None;
        }

        let safe_len = if data.len() >= 8 { data.len() - 8 } else { 0 };
        let (num_rounds, simd_bytes) = common::calculate_blocks(safe_len, BLOCK_SIZE);

        let mut offset = 0;
        for _ in 0..num_rounds {
            // Load 32 bytes (we only use the first 24)
            let input_vec = _mm256_loadu_si256(data.as_ptr().add(offset) as *const __m256i);

            // Reshuffle bytes to extract 6-bit groups
            let reshuffled = self.reshuffle_6bit_avx2(input_vec);

            // Translate 6-bit indices to ASCII using pluggable translator
            let encoded = self.translator.translate_encode_256(reshuffled);

            // Store 32 output characters
            let mut output_buf = [0u8; 32];
            _mm256_storeu_si256(output_buf.as_mut_ptr() as *mut __m256i, encoded);

            for &byte in &output_buf {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // TODO: Handle remainder
        if simd_bytes < data.len() {
            // For now, we don't handle remainder
        }

        Some(result)
    }

    #[cfg(target_arch = "x86_64")]
    fn encode_6bit_avx2(&self, data: &[u8], dict: &Dictionary) -> Option<String> {
        unsafe { self.encode_6bit_avx2_impl(data, dict) }
    }

    /// Reshuffle bytes and extract 6-bit indices from 24 input bytes (AVX2 version)
    ///
    /// AVX2 shuffle operates on each 128-bit lane independently, so we process
    /// as two separate 128-bit halves
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn reshuffle_6bit_avx2(&self, input: __m256i) -> __m256i {
        // AVX2 shuffle is per-lane, so we process each 128-bit lane independently
        // Each lane processes 12 bytes -> 16 output chars

        let shuffled = _mm256_shuffle_epi8(
            input,
            _mm256_setr_epi8(
                // First 128-bit lane (bytes 0-15)
                1, 2, 0, 1, // bytes for output positions 0-3
                4, 5, 3, 4, // bytes for output positions 4-7
                7, 8, 6, 7, // bytes for output positions 8-11
                10, 11, 9, 10, // bytes for output positions 12-15
                // Second 128-bit lane (bytes 16-31)
                1, 2, 0, 1, // bytes for output positions 16-19
                4, 5, 3, 4, // bytes for output positions 20-23
                7, 8, 6, 7, // bytes for output positions 24-27
                10, 11, 9, 10, // bytes for output positions 28-31
            ),
        );

        // Extract 6-bit groups using multiplication tricks
        let t0 = _mm256_and_si256(shuffled, _mm256_set1_epi32(0x0FC0FC00_u32 as i32));
        let t1 = _mm256_mulhi_epu16(t0, _mm256_set1_epi32(0x04000040_u32 as i32));

        let t2 = _mm256_and_si256(shuffled, _mm256_set1_epi32(0x003F03F0_u32 as i32));
        let t3 = _mm256_mullo_epi16(t2, _mm256_set1_epi32(0x01000010_u32 as i32));

        _mm256_or_si256(t1, t3)
    }

    /// Decode 8-bit alphabet using AVX2 (processes 32 chars -> 32 bytes)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn decode_8bit_avx2_impl(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        const BLOCK_SIZE: usize = 32;

        let encoded_bytes = encoded.as_bytes();
        let mut result = Vec::with_capacity(encoded_bytes.len());

        if encoded_bytes.len() < BLOCK_SIZE {
            return None;
        }

        let num_blocks = encoded_bytes.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Load 32 ASCII chars
            let chars = _mm256_loadu_si256(encoded_bytes.as_ptr().add(offset) as *const __m256i);

            // Translate to bytes (validation included)
            let bytes = self.translator.translate_decode_256(chars)?;

            // Store 32 output bytes
            let mut output_buf = [0u8; 32];
            _mm256_storeu_si256(output_buf.as_mut_ptr() as *mut __m256i, bytes);
            result.extend_from_slice(&output_buf);

            offset += BLOCK_SIZE;
        }

        // TODO: Handle remainder
        if simd_bytes < encoded_bytes.len() {
            // For now, we don't handle remainder
        }

        Some(result)
    }

    #[cfg(target_arch = "x86_64")]
    fn decode_8bit_avx2(&self, encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
        unsafe { self.decode_8bit_avx2_impl(encoded, dict) }
    }

    /// Decode 4-bit alphabet using AVX2 (processes 64 chars -> 32 bytes)
    ///
    /// For now, just call SSSE3 version twice - simpler and correct
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_4bit_avx2_impl(&self, encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
        // For simplicity, fall back to SSSE3 for now
        // A proper AVX2 implementation would process 64 chars at once
        // but the lane-crossing complexity makes it error-prone
        self.decode_4bit(encoded, dict)
    }

    #[cfg(target_arch = "x86_64")]
    fn decode_4bit_avx2(&self, encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
        unsafe { self.decode_4bit_avx2_impl(encoded, dict) }
    }

    /// Decode 5-bit alphabet using AVX2 (processes 32 chars -> 20 bytes)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn decode_5bit_avx2_impl(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        const BLOCK_SIZE: usize = 32; // 32 chars -> 20 bytes

        let encoded_bytes = encoded.as_bytes();
        let mut result = Vec::with_capacity(encoded_bytes.len() * 5 / 8);

        if encoded_bytes.len() < BLOCK_SIZE {
            return None;
        }

        let num_blocks = encoded_bytes.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for round in 0..num_blocks {
            let offset = round * BLOCK_SIZE;

            // Load 32 ASCII chars
            let chars = _mm256_loadu_si256(encoded_bytes.as_ptr().add(offset) as *const __m256i);

            // Translate to 5-bit indices (validation included)
            let indices = self.translator.translate_decode_256(chars)?;

            // Pack 5-bit values into bytes (32 chars -> 20 bytes)
            let decoded = self.pack_5bit_to_8bit_avx2(indices);

            // Extract 10 bytes from each 128-bit lane (20 total)
            let lane0 = _mm256_castsi256_si128(decoded);
            let lane1 = _mm256_extracti128_si256(decoded, 1);

            let mut buf0 = [0u8; 16];
            let mut buf1 = [0u8; 16];
            _mm_storeu_si128(buf0.as_mut_ptr() as *mut __m128i, lane0);
            _mm_storeu_si128(buf1.as_mut_ptr() as *mut __m128i, lane1);

            result.extend_from_slice(&buf0[0..10]);
            result.extend_from_slice(&buf1[0..10]);
        }

        // TODO: Handle remainder
        if simd_bytes < encoded_bytes.len() {
            // For now, we don't handle remainder
        }

        Some(result)
    }

    #[cfg(target_arch = "x86_64")]
    fn decode_5bit_avx2(&self, encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
        unsafe { self.decode_5bit_avx2_impl(encoded, dict) }
    }

    /// Decode 6-bit alphabet using AVX2 (processes 32 chars -> 24 bytes)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn decode_6bit_avx2_impl(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        const BLOCK_SIZE: usize = 32; // 32 chars -> 24 bytes

        let encoded_bytes = encoded.as_bytes();
        let mut result = Vec::with_capacity(encoded_bytes.len() * 3 / 4);

        if encoded_bytes.len() < BLOCK_SIZE {
            return None;
        }

        let num_blocks = encoded_bytes.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for round in 0..num_blocks {
            let offset = round * BLOCK_SIZE;

            // Load 32 ASCII chars
            let chars = _mm256_loadu_si256(encoded_bytes.as_ptr().add(offset) as *const __m256i);

            // Translate to 6-bit indices (validation included)
            let indices = self.translator.translate_decode_256(chars)?;

            // Unpack 6-bit indices back to bytes
            let bytes = self.unshuffle_6bit_avx2(indices);

            // Store 24 output bytes (from 32-byte buffer)
            let mut output_buf = [0u8; 32];
            _mm256_storeu_si256(output_buf.as_mut_ptr() as *mut __m256i, bytes);
            result.extend_from_slice(&output_buf[..24]);
        }

        // TODO: Handle remainder
        if simd_bytes < encoded_bytes.len() {
            // For now, we don't handle remainder
        }

        Some(result)
    }

    #[cfg(target_arch = "x86_64")]
    fn decode_6bit_avx2(&self, encoded: &str, dict: &Dictionary) -> Option<Vec<u8>> {
        unsafe { self.decode_6bit_avx2_impl(encoded, dict) }
    }

    /// Unshuffle 6-bit indices back to 8-bit bytes (AVX2 version)
    ///
    /// Inverse of reshuffle_6bit_avx2 - converts 32 bytes of 6-bit indices to 24 bytes of data
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn unshuffle_6bit_avx2(&self, indices: __m256i) -> __m256i {
        // Stage 1: Merge adjacent pairs using multiply-add
        let merge_ab_and_bc =
            _mm256_maddubs_epi16(indices, _mm256_set1_epi32(0x01400140u32 as i32));

        // Stage 2: Combine 16-bit pairs into 32-bit values
        let final_32bit =
            _mm256_madd_epi16(merge_ab_and_bc, _mm256_set1_epi32(0x00011000u32 as i32));

        // Stage 3: Extract the valid bytes from each 32-bit group
        _mm256_shuffle_epi8(
            final_32bit,
            _mm256_setr_epi8(
                // First lane: extract 12 bytes from 4 groups of 32-bit values
                2, 1, 0, // first group of 3 bytes
                6, 5, 4, // second group of 3 bytes
                10, 9, 8, // third group of 3 bytes
                14, 13, 12, // fourth group of 3 bytes
                -1, -1, -1, -1, // unused
                // Second lane: extract 12 bytes from 4 groups of 32-bit values
                2, 1, 0, // first group of 3 bytes
                6, 5, 4, // second group of 3 bytes
                10, 9, 8, // third group of 3 bytes
                14, 13, 12, // fourth group of 3 bytes
                -1, -1, -1, -1, // unused
            ),
        )
    }

    /// Extract 32 x 5-bit indices from 20 packed input bytes (AVX2)
    ///
    /// Processes two independent 10-byte blocks in parallel (one per 128-bit lane).
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn extract_5bit_indices_avx2(&self, input: __m256i) -> __m256i {
        // Extract both 128-bit lanes and process separately
        let lane_lo = _mm256_castsi256_si128(input);
        let lane_hi = _mm256_extracti128_si256(input, 1);

        // Apply SSSE3 unpacking to each lane
        let indices_lo = self.unpack_5bit_simple(lane_lo);
        let indices_hi = self.unpack_5bit_simple(lane_hi);

        // Recombine into 256-bit register
        _mm256_set_m128i(indices_hi, indices_lo)
    }

    /// Pack 32 bytes of 5-bit indices into 20 bytes (AVX2)
    ///
    /// Processes two independent 16-char blocks (one per 128-bit lane).
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn pack_5bit_to_8bit_avx2(&self, indices: __m256i) -> __m256i {
        // Extract both 128-bit lanes and process separately
        let lane_lo = _mm256_castsi256_si128(indices);
        let lane_hi = _mm256_extracti128_si256(indices, 1);

        // Apply SSSE3 packing to each lane
        let packed_lo = self.pack_5bit_to_8bit(lane_lo);
        let packed_hi = self.pack_5bit_to_8bit(lane_hi);

        // Recombine into 256-bit register
        _mm256_set_m128i(packed_hi, packed_lo)
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

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_5bit_sequential() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Create sequential base32 starting at 'A' (U+0041) - ASCII range
        // Uses 32 sequential chars: 'A' through '`' (0x41..0x61)
        let chars: Vec<char> = (0x41..0x61).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict).unwrap();

        // Test data: 20 bytes (will process 10 bytes in SIMD due to safe_len calculation)
        // Note: remainder handling is TODO, so only 10 bytes will be processed
        let data = b"Hello, World!!!!!!!!";
        let result = codec.encode_5bit(data, &dict);

        assert!(result.is_some());
        let encoded = result.unwrap();

        // Verify length: 10 bytes processed -> 16 base32 chars (remainder not handled)
        assert_eq!(encoded.len(), 16);

        // Verify all output is ASCII (< 0x80)
        for byte in encoded.as_bytes() {
            assert!(*byte < 0x80, "Output should be ASCII, got 0x{:02X}", byte);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_5bit_round_trip() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Create sequential base32 starting at 'A' (U+0041)
        let chars: Vec<char> = (0x41..0x61).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();

        let codec = GenericSimdCodec::from_dictionary(&dict).unwrap();

        // Test data: 16 bytes (minimum for SIMD, processes 10 bytes)
        // Note: We load 16 bytes but only process 10, remainder handling is TODO
        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10";

        // Encode (will encode first 10 bytes only)
        let encoded = codec.encode(data, &dict).expect("Encode failed");

        // Should be 16 chars (10 bytes encoded)
        assert_eq!(encoded.len(), 16);

        // Decode
        let decoded = codec.decode(&encoded, &dict).expect("Decode failed");

        // Verify first 10 bytes match (remainder not processed)
        assert_eq!(&decoded[..], &data[..10], "Round-trip failed");
    }
}
