//! SIMD translation abstractions for encoding/decoding
//!
//! This module provides the translation layer that converts between
//! dictionary indices and characters during SIMD encoding/decoding.
//!
//! The key insight: for sequential dictionaries (contiguous Unicode ranges),
//! translation is a single SIMD add/subtract instruction.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// SIMD translation operations for encoding/decoding
///
/// This trait abstracts the translation layer, allowing different
/// dictionary structures to plug into the same reshuffle algorithms.
///
/// # Safety
/// All methods in this trait require SIMD support (SSSE3 on x86_64, NEON on aarch64)
/// and must only be called within a function marked with appropriate target_feature.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[allow(dead_code)]
pub trait SimdTranslate {
    /// Translate indices to characters (encoding)
    ///
    /// Takes a vector of dictionary indices (e.g., 0-63 for base64)
    /// and converts them to their corresponding character codepoints.
    ///
    /// # Safety
    /// Caller must verify SIMD support before calling
    #[cfg(target_arch = "x86_64")]
    unsafe fn translate_encode(&self, indices: __m128i) -> __m128i;

    /// Translate indices to characters (encoding) - NEON version
    ///
    /// Takes a vector of dictionary indices (e.g., 0-63 for base64)
    /// and converts them to their corresponding character codepoints.
    ///
    /// # Safety
    /// Caller must verify NEON support before calling
    #[cfg(target_arch = "aarch64")]
    unsafe fn translate_encode(&self, indices: uint8x16_t) -> uint8x16_t;

    /// Translate characters to indices (decoding)
    ///
    /// Takes a vector of character bytes and converts them to
    /// dictionary indices. Returns None if invalid characters detected.
    ///
    /// # Safety
    /// Caller must verify SIMD support before calling
    #[cfg(target_arch = "x86_64")]
    unsafe fn translate_decode(&self, chars: __m128i) -> Option<__m128i>;

    /// Translate characters to indices (decoding) - NEON version
    ///
    /// Takes a vector of character bytes and converts them to
    /// dictionary indices. Returns None if invalid characters detected.
    ///
    /// # Safety
    /// Caller must verify NEON support before calling
    #[cfg(target_arch = "aarch64")]
    unsafe fn translate_decode(&self, chars: uint8x16_t) -> Option<uint8x16_t>;

    /// Validate that all characters are valid for this dictionary
    ///
    /// # Safety
    /// Caller must verify SIMD support before calling
    #[cfg(target_arch = "x86_64")]
    unsafe fn validate(&self, chars: __m128i) -> bool;

    /// Validate that all characters are valid for this dictionary - NEON version
    ///
    /// # Safety
    /// Caller must verify NEON support before calling
    #[cfg(target_arch = "aarch64")]
    unsafe fn validate(&self, chars: uint8x16_t) -> bool;

    /// Translate indices to characters (encoding) - AVX2 version
    ///
    /// Takes a 256-bit vector of dictionary indices and converts them
    /// to their corresponding character codepoints.
    ///
    /// # Safety
    /// Caller must verify AVX2 support before calling
    #[cfg(target_arch = "x86_64")]
    unsafe fn translate_encode_256(&self, indices: __m256i) -> __m256i;

    /// Translate characters to indices (decoding) - AVX2 version
    ///
    /// Takes a 256-bit vector of character bytes and converts them to
    /// dictionary indices. Returns None if invalid characters detected.
    ///
    /// # Safety
    /// Caller must verify AVX2 support before calling
    #[cfg(target_arch = "x86_64")]
    unsafe fn translate_decode_256(&self, chars: __m256i) -> Option<__m256i>;
}

/// SIMD translation for sequential dictionaries (zero-cost)
///
/// This is the ideal case: dictionaries where characters are contiguous
/// in Unicode, enabling translation with a single add/subtract instruction.
///
/// # Examples
/// - Base64 starting at '@': U+0040..U+007F (64 chars)
/// - ASCII digits: U+0030..U+0039 (10 chars)
/// - Lowercase hex: U+0061..U+006A ('a'..'j', if we wanted base10)
///
/// # Performance
/// Encoding: single `paddb` (add) instruction
/// Decoding: single `psubb` (subtract) + range check
#[derive(Debug, Clone, Copy)]
pub struct SequentialTranslate {
    /// Starting codepoint of the dictionary
    start_codepoint: u32,
    /// Number of bits per symbol (determines valid range)
    bits_per_symbol: u8,
}

impl SequentialTranslate {
    /// Create a new sequential translator
    ///
    /// # Arguments
    /// * `start_codepoint` - The Unicode codepoint of index 0
    /// * `bits_per_symbol` - Number of bits per symbol (e.g., 6 for base64)
    ///
    /// # Example
    /// ```ignore
    /// // Base64 dictionary starting at '@' (U+0040)
    /// let translator = SequentialTranslate::new(0x40, 6);
    /// ```
    #[allow(dead_code)]
    pub const fn new(start_codepoint: u32, bits_per_symbol: u8) -> Self {
        Self {
            start_codepoint,
            bits_per_symbol,
        }
    }

    /// Get the starting codepoint
    #[allow(dead_code)]
    pub const fn start_codepoint(&self) -> u32 {
        self.start_codepoint
    }

    /// Get bits per symbol
    #[allow(dead_code)]
    pub const fn bits_per_symbol(&self) -> u8 {
        self.bits_per_symbol
    }

    /// Maximum valid index for this dictionary
    #[allow(dead_code)]
    const fn max_index(&self) -> u8 {
        (1u8 << self.bits_per_symbol) - 1
    }
}

#[cfg(target_arch = "x86_64")]
impl SimdTranslate for SequentialTranslate {
    #[target_feature(enable = "ssse3")]
    unsafe fn translate_encode(&self, indices: __m128i) -> __m128i {
        // Zero-cost translation: single vector add
        // indices + start_codepoint = characters
        //
        // Example: base64 starting at '@' (0x40)
        // Index 0 → 0 + 0x40 = '@'
        // Index 1 → 1 + 0x40 = 'A'
        // Index 63 → 63 + 0x40 = '\x7F'
        //
        // Compiles to: paddb xmm0, xmm1
        let offset = _mm_set1_epi8(self.start_codepoint as i8);
        _mm_add_epi8(indices, offset)
    }

    #[target_feature(enable = "ssse3")]
    unsafe fn translate_decode(&self, chars: __m128i) -> Option<__m128i> {
        // Reverse translation: subtract start_codepoint
        // chars - start_codepoint = indices
        let offset = _mm_set1_epi8(self.start_codepoint as i8);
        let indices = _mm_sub_epi8(chars, offset);

        // Validate range: all indices must be < (1 << bits_per_symbol)
        // We check that indices <= max_index using unsigned comparison
        let max_valid = _mm_set1_epi8(self.max_index() as i8);

        // For unsigned comparison: indices > max_valid means invalid
        // We compare as unsigned by adding 128 to both sides
        let bias = _mm_set1_epi8(-128_i8);
        let indices_biased = _mm_add_epi8(indices, bias);
        let max_biased = _mm_add_epi8(max_valid, bias);

        // Check indices_biased <= max_biased (unsigned)
        let too_large = _mm_cmpgt_epi8(indices_biased, max_biased);
        let invalid_mask = _mm_movemask_epi8(too_large);

        if invalid_mask == 0 {
            Some(indices)
        } else {
            None
        }
    }

    #[target_feature(enable = "ssse3")]
    unsafe fn validate(&self, chars: __m128i) -> bool {
        // Validate that all characters are in [start, start + 2^bits)
        let start = _mm_set1_epi8(self.start_codepoint as i8);

        // Check: start <= chars < end
        // For chars >= start: chars - start >= 0 (unsigned)
        let chars_offset = _mm_sub_epi8(chars, start);
        let range_size = _mm_set1_epi8((1 << self.bits_per_symbol) as i8);

        // Unsigned comparison: chars_offset < range_size
        let bias = _mm_set1_epi8(-128_i8);
        let offset_biased = _mm_add_epi8(chars_offset, bias);
        let range_biased = _mm_add_epi8(range_size, bias);

        let too_large = _mm_cmpgt_epi8(offset_biased, _mm_sub_epi8(range_biased, _mm_set1_epi8(1)));
        let invalid_mask = _mm_movemask_epi8(too_large);

        invalid_mask == 0
    }

    #[target_feature(enable = "avx2")]
    unsafe fn translate_encode_256(&self, indices: __m256i) -> __m256i {
        // Zero-cost translation: single vector add (AVX2 version)
        let offset = _mm256_set1_epi8(self.start_codepoint as i8);
        _mm256_add_epi8(indices, offset)
    }

    #[target_feature(enable = "avx2")]
    unsafe fn translate_decode_256(&self, chars: __m256i) -> Option<__m256i> {
        // Reverse translation: subtract start_codepoint
        let offset = _mm256_set1_epi8(self.start_codepoint as i8);
        let indices = _mm256_sub_epi8(chars, offset);

        // Validate range: all indices must be < (1 << bits_per_symbol)
        let max_valid = _mm256_set1_epi8(self.max_index() as i8);

        // For unsigned comparison: indices > max_valid means invalid
        let bias = _mm256_set1_epi8(-128_i8);
        let indices_biased = _mm256_add_epi8(indices, bias);
        let max_biased = _mm256_add_epi8(max_valid, bias);

        // Check indices_biased <= max_biased (unsigned)
        let too_large = _mm256_cmpgt_epi8(indices_biased, max_biased);
        let invalid_mask = _mm256_movemask_epi8(too_large);

        if invalid_mask == 0 {
            Some(indices)
        } else {
            None
        }
    }
}

#[cfg(target_arch = "aarch64")]
impl SimdTranslate for SequentialTranslate {
    #[target_feature(enable = "neon")]
    unsafe fn translate_encode(&self, indices: uint8x16_t) -> uint8x16_t {
        // Zero-cost translation: single vector add
        // indices + start_codepoint = characters
        //
        // Example: base64 starting at '@' (0x40)
        // Index 0 → 0 + 0x40 = '@'
        // Index 1 → 1 + 0x40 = 'A'
        // Index 63 → 63 + 0x40 = '\x7F'
        //
        // NEON: vaddq_u8
        let offset = vdupq_n_u8(self.start_codepoint as u8);
        vaddq_u8(indices, offset)
    }

    #[target_feature(enable = "neon")]
    unsafe fn translate_decode(&self, chars: uint8x16_t) -> Option<uint8x16_t> {
        // Reverse translation: subtract start_codepoint
        // chars - start_codepoint = indices
        let offset = vdupq_n_u8(self.start_codepoint as u8);
        let indices = vsubq_u8(chars, offset);

        // Validate range: all indices must be <= max_index
        let max_valid = vdupq_n_u8(self.max_index());

        // NEON unsigned comparison: indices > max_valid
        let too_large = vcgtq_u8(indices, max_valid);

        // Check if any lane is invalid (vmaxvq returns max across all lanes)
        let invalid = vmaxvq_u8(too_large);

        if invalid == 0 {
            Some(indices)
        } else {
            None
        }
    }

    #[target_feature(enable = "neon")]
    unsafe fn validate(&self, chars: uint8x16_t) -> bool {
        // Validate that all characters are in [start, start + 2^bits)
        let start = vdupq_n_u8(self.start_codepoint as u8);

        // Check: start <= chars < end
        // For chars >= start: chars - start >= 0 (unsigned, wraps if chars < start)
        let chars_offset = vsubq_u8(chars, start);
        let max_valid = vdupq_n_u8(self.max_index());

        // Unsigned comparison: chars_offset > max_valid
        let too_large = vcgtq_u8(chars_offset, max_valid);

        // Check if any lane is invalid
        let invalid = vmaxvq_u8(too_large);

        invalid == 0
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "x86_64")]
    use super::{SequentialTranslate, SimdTranslate};

    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_sequential_encode_at_sign() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        // Base64 starting at '@' (0x40)
        let translator = SequentialTranslate::new(0x40, 6);

        unsafe {
            // Test indices 0-15
            let indices = _mm_setr_epi8(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);

            let chars = translator.translate_encode(indices);

            // Extract bytes for verification
            let mut result = [0u8; 16];
            _mm_storeu_si128(result.as_mut_ptr() as *mut __m128i, chars);

            // Expected: '@', 'A', 'B', 'C', ...
            let expected = [
                0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D,
                0x4E, 0x4F,
            ];

            assert_eq!(result, expected);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_sequential_decode_at_sign() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        let translator = SequentialTranslate::new(0x40, 6);

        unsafe {
            // Characters '@' through 'O'
            let chars = _mm_setr_epi8(
                0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D,
                0x4E, 0x4F,
            );

            let indices = translator
                .translate_decode(chars)
                .expect("Valid characters");

            // Extract bytes
            let mut result = [0u8; 16];
            _mm_storeu_si128(result.as_mut_ptr() as *mut __m128i, indices);

            // Expected: 0-15
            let expected = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

            assert_eq!(result, expected);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_sequential_decode_invalid() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        let translator = SequentialTranslate::new(0x40, 6); // Valid: 0x40..0x7F (64 chars)

        unsafe {
            // Characters beyond valid range
            let chars = _mm_setr_epi8(
                0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D,
                0x4E, -0x7F_i8, // Last byte is 0x81 - invalid
            );

            let result = translator.translate_decode(chars);
            assert!(result.is_none(), "Should reject invalid characters");
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_sequential_validate_valid() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        let translator = SequentialTranslate::new(0x30, 4); // '0'..'9' + 6 more

        unsafe {
            // Valid characters in range 0x30..0x3F
            let chars = _mm_setr_epi8(
                0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D,
                0x3E, 0x3F,
            );

            assert!(translator.validate(chars));
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_sequential_validate_invalid() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        let translator = SequentialTranslate::new(0x30, 4); // Valid: 0x30..0x3F

        unsafe {
            // One character out of range
            let chars = _mm_setr_epi8(
                0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D,
                0x3E, 0x40, // 0x40 is beyond range
            );

            assert!(!translator.validate(chars));
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_sequential_round_trip() {
        if !crate::simd::has_ssse3() {
            eprintln!("SSSE3 not available, skipping test");
            return;
        }

        let translator = SequentialTranslate::new(0x41, 6); // 'A'..'`' (64 chars)

        unsafe {
            // Original indices
            let original_indices =
                _mm_setr_epi8(0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 63, 0, 0);

            // Encode to characters
            let chars = translator.translate_encode(original_indices);

            // Decode back to indices
            let decoded_indices = translator
                .translate_decode(chars)
                .expect("Valid round trip");

            // Extract and compare
            let mut original = [0u8; 16];
            let mut decoded = [0u8; 16];
            _mm_storeu_si128(original.as_mut_ptr() as *mut __m128i, original_indices);
            _mm_storeu_si128(decoded.as_mut_ptr() as *mut __m128i, decoded_indices);

            assert_eq!(original, decoded);
        }
    }
}
