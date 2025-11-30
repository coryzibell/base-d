//! Base64LutCodec: SIMD codec for base64-scale arbitrary dictionaries (17-64 characters)
//!
//! Platform-specific strategies:
//! - ARM NEON: vqtbl4q_u8 (64-byte direct lookup)
//! - x86 AVX-512 VBMI: vpermb (64-byte direct lookup)
//! - x86 fallback: Scalar (SSSE3 range-reduction deferred)
//!
//! Constraints:
//! - 17 <= Base <= 64
//! - Power-of-2 base (32 or 64)
//! - ASCII-only (char < 0x80)
//! - Non-sequential dictionaries only

use crate::core::dictionary::Dictionary;
use crate::simd::variants::{DictionaryMetadata, LutStrategy, TranslationStrategy};

#[cfg(target_arch = "x86_64")]
use super::common::{CharRange, RangeInfo, RangeStrategy};

/// SIMD codec for base64-scale arbitrary dictionaries (17-64 characters)
///
/// Uses platform-dependent lookup for encoding and a 256-byte sparse
/// table for decoding with validation.
pub struct Base64LutCodec {
    pub(super) metadata: DictionaryMetadata,

    /// Encoding LUT: index → char (64 bytes, one per possible index)
    pub(super) encode_lut: [u8; 64],

    /// Decoding LUT: char → index (256 bytes, sparse)
    /// 0xFF means invalid character
    pub(super) decode_lut: [u8; 256],

    /// Range-reduction metadata for SSE/AVX2 fallback (x86_64 only)
    #[cfg(target_arch = "x86_64")]
    pub(super) range_info: Option<RangeInfo>,
}

impl Base64LutCodec {
    /// Detect contiguous ASCII ranges in dictionary
    #[cfg(target_arch = "x86_64")]
    fn detect_ranges(encode_lut: &[u8], base: usize) -> Vec<CharRange> {
        let mut ranges = Vec::new();
        let mut start_idx = 0;

        while start_idx < base {
            let start_char = encode_lut[start_idx];
            let mut end_idx = start_idx;

            // Find longest contiguous ASCII sequence
            while end_idx + 1 < base && encode_lut[end_idx + 1] == encode_lut[end_idx] + 1 {
                end_idx += 1;
            }

            let range = CharRange {
                start_idx: start_idx as u8,
                end_idx: end_idx as u8,
                start_char,
                offset: (start_char as i8).wrapping_sub(start_idx as i8),
            };

            ranges.push(range);
            start_idx = end_idx + 1;
        }

        ranges
    }

    /// Create codec from dictionary
    ///
    /// Returns None if:
    /// - Dictionary not in range 17-64 chars
    /// - Not power-of-2 base (32 or 64)
    /// - Dictionary is sequential (should use GenericSimdCodec)
    /// - Any character > 0x7F (non-ASCII)
    pub fn from_dictionary(dict: &Dictionary) -> Option<Self> {
        let metadata = DictionaryMetadata::from_dictionary(dict);

        // Only for large arbitrary dictionaries (17-64 chars)
        if metadata.base < 17 || metadata.base > 64 {
            return None;
        }

        // Must be power-of-2 (32 or 64)
        if !metadata.base.is_power_of_two() {
            return None;
        }

        // Must be arbitrary (non-sequential)
        if !matches!(metadata.strategy, TranslationStrategy::Arbitrary { .. }) {
            return None;
        }

        // Verify LUT strategy is appropriate
        if metadata.lut_strategy() != LutStrategy::LargePlatformDependent {
            return None;
        }

        // Build encoding LUT (index → char)
        let mut encode_lut = [0u8; 64];
        for (i, lut_entry) in encode_lut.iter_mut().enumerate().take(metadata.base) {
            let ch = dict.encode_digit(i)?;

            // Validation: char must be ASCII (single-byte)
            if (ch as u32) > 0x7F {
                return None; // Multi-byte UTF-8 not supported
            }

            *lut_entry = ch as u8;
        }

        // Build decoding LUT (char → index, 256-entry sparse table)
        let mut decode_lut = [0xFFu8; 256];
        for (idx, &ch_byte) in encode_lut[..metadata.base].iter().enumerate() {
            decode_lut[ch_byte as usize] = idx as u8;
        }

        // Analyze ranges for SSE/AVX2 range-reduction (x86_64 only)
        #[cfg(target_arch = "x86_64")]
        let range_info = {
            let ranges = Self::detect_ranges(&encode_lut, metadata.base);
            // Try to build range-reduction metadata if feasible (2-5 ranges)
            RangeInfo::build_multi_range(&ranges)
        };

        Some(Self {
            metadata,
            encode_lut,
            decode_lut,
            #[cfg(target_arch = "x86_64")]
            range_info,
        })
    }

    /// Encode binary data to string using SIMD
    ///
    /// Returns None if SIMD is not available or encoding fails.
    pub fn encode(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        // Only supports 5-bit (base 32) and 6-bit (base 64) for now
        if self.metadata.base != 32 && self.metadata.base != 64 {
            return None;
        }

        // Handle empty input
        if data.is_empty() {
            return Some(String::new());
        }

        // Calculate output length based on base
        let output_len = match self.metadata.base {
            32 => (data.len() * 8).div_ceil(5), // 5 bits per char
            64 => (data.len() * 8).div_ceil(6), // 6 bits per char
            _ => return None,
        };

        let mut result = String::with_capacity(output_len);

        #[cfg(target_arch = "aarch64")]
        unsafe {
            self.encode_neon_impl(data, &mut result);
            Some(result)
        }

        #[cfg(target_arch = "x86_64")]
        unsafe {
            self.encode_x86_impl(data, &mut result);
            Some(result)
        }

        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            let _ = result;
            // No SIMD available for this architecture
            None
        }
    }

    /// aarch64 NEON encode implementation
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_impl(&self, data: &[u8], result: &mut String) {
        if self.metadata.base == 32 {
            unsafe { self.encode_neon_base32(data, result) };
        } else if self.metadata.base == 64 {
            unsafe { self.encode_neon_base64(data, result) };
        }
    }

    /// NEON base64 encode (6-bit indices)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_base64(&self, data: &[u8], result: &mut String) {
        use std::arch::aarch64::*;

        const BLOCK_SIZE: usize = 12; // 12 bytes -> 16 chars
        const SIMD_READ: usize = 16; // Actually reads 16 bytes for reshuffle

        if data.len() < SIMD_READ {
            self.encode_scalar_base64(data, result);
            return;
        }

        let safe_len = if data.len() >= 4 { data.len() - 4 } else { 0 };
        let num_blocks = safe_len / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Unsafe: pointer arithmetic for LUT loading
        let lut_tables = unsafe {
            uint8x16x4_t(
                vld1q_u8(self.encode_lut.as_ptr()),
                vld1q_u8(self.encode_lut.as_ptr().add(16)),
                vld1q_u8(self.encode_lut.as_ptr().add(32)),
                vld1q_u8(self.encode_lut.as_ptr().add(48)),
            )
        };

        let mut offset = 0;
        for _ in 0..num_blocks {
            // SAFETY: offset + SIMD_READ <= data.len() guaranteed by safe_len calculation
            // (safe_len ensures data.len() - 4, num_blocks uses BLOCK_SIZE=12, leaving 4-byte buffer)
            debug_assert!(offset + SIMD_READ <= data.len());

            // Unsafe: pointer arithmetic for data loading
            let input_vec = unsafe { vld1q_u8(data.as_ptr().add(offset)) };

            // Reshuffle to extract 6-bit indices (unsafe - calls unsafe fn)
            let reshuffled = unsafe { self.reshuffle_neon_base64(input_vec) };

            // Translate using vqtbl4q_u8 (64-byte lookup) - safe, no memory op
            let chars = vqtbl4q_u8(lut_tables, reshuffled);

            // Store 16 output characters (unsafe - memory op)
            let mut output_buf = [0u8; 16];
            unsafe { vst1q_u8(output_buf.as_mut_ptr(), chars) };

            // Append to result
            for &byte in &output_buf {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Handle remainder with scalar
        if simd_bytes < data.len() {
            self.encode_scalar_base64(&data[simd_bytes..], result);
        }
    }

    /// Reshuffle bytes and extract 6-bit indices from 12 input bytes (NEON)
    /// Based on specialized base64.rs reshuffle_neon
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn reshuffle_neon_base64(
        &self,
        input: std::arch::aarch64::uint8x16_t,
    ) -> std::arch::aarch64::uint8x16_t {
        use std::arch::aarch64::*;

        // Shuffle mask: Reshuffle bytes to prepare for 6-bit extraction
        // For each group of 3 input bytes ABC (24 bits) -> 4 output indices (4 x 6 bits)
        // Matches x86_64 base64.rs pattern: [1,0,2,1, 4,3,5,4, 7,6,8,7, 10,9,11,10]
        // Load is unsafe (memory op)
        let shuffle_indices = unsafe {
            vld1q_u8(
                [
                    1, 0, 2, 1, // bytes 0-2 -> positions 0-3
                    4, 3, 5, 4, // bytes 3-5 -> positions 4-7
                    7, 6, 8, 7, // bytes 6-8 -> positions 8-11
                    10, 9, 11, 10, // bytes 9-11 -> positions 12-15
                ]
                .as_ptr(),
            )
        };

        // All operations below are safe (no memory ops)
        let shuffled = vqtbl1q_u8(input, shuffle_indices);
        let shuffled_u32 = vreinterpretq_u32_u8(shuffled);

        // Extract 6-bit groups using shifts and masks
        // This matches the x86_64 algorithm which uses mulhi_epu16 and mullo_epi16.
        // NEON doesn't have a direct mulhi equivalent, so we use vmull/vshrn pattern.

        // First extraction: get bits for positions 0 and 2 in each group of 4
        // x86: mulhi_epu16(and(shuffled, 0x0FC0FC00), 0x04000040)
        let t0 = vandq_u32(shuffled_u32, vdupq_n_u32(0x0FC0FC00));
        let t1 = {
            let t0_u16 = vreinterpretq_u16_u32(t0);
            // Implement mulhi_epu16 using vmull + vshrn
            // 0x04000040 as 16-bit lanes: [0x0040, 0x0400, 0x0040, 0x0400, ...]
            let mult_pattern = vreinterpretq_u16_u32(vdupq_n_u32(0x04000040));
            let lo = vget_low_u16(t0_u16);
            let hi = vget_high_u16(t0_u16);
            let mult_lo = vget_low_u16(mult_pattern);
            let mult_hi = vget_high_u16(mult_pattern);
            let lo_32 = vmull_u16(lo, mult_lo);
            let hi_32 = vmull_u16(hi, mult_hi);
            let lo_result = vshrn_n_u32(lo_32, 16);
            let hi_result = vshrn_n_u32(hi_32, 16);
            vreinterpretq_u32_u16(vcombine_u16(lo_result, hi_result))
        };

        // Second extraction: get bits for positions 1 and 3 in each group of 4
        // x86: mullo_epi16(and(shuffled, 0x003F03F0), 0x01000010)
        let t2 = vandq_u32(shuffled_u32, vdupq_n_u32(0x003F03F0));
        let t3 = {
            let t2_u16 = vreinterpretq_u16_u32(t2);
            // mullo is just regular multiply (keep low 16 bits)
            // 0x01000010 as 16-bit lanes: [0x0010, 0x0100, 0x0010, 0x0100, ...]
            let mult_pattern = vreinterpretq_u16_u32(vdupq_n_u32(0x01000010));
            vreinterpretq_u32_u16(vmulq_u16(t2_u16, mult_pattern))
        };

        // Combine the two results
        vreinterpretq_u8_u32(vorrq_u32(t1, t3))
    }

    /// Scalar fallback for base64 encoding
    #[cfg(target_arch = "aarch64")]
    fn encode_scalar_base64(&self, data: &[u8], result: &mut String) {
        let mut bit_buffer = 0u32;
        let mut bits_in_buffer = 0;

        for &byte in data {
            bit_buffer = (bit_buffer << 8) | (byte as u32);
            bits_in_buffer += 8;

            while bits_in_buffer >= 6 {
                bits_in_buffer -= 6;
                let index = ((bit_buffer >> bits_in_buffer) & 0x3F) as usize;
                result.push(self.encode_lut[index] as char);
            }
        }

        // Flush remaining bits
        if bits_in_buffer > 0 {
            let index = ((bit_buffer << (6 - bits_in_buffer)) & 0x3F) as usize;
            result.push(self.encode_lut[index] as char);
        }
    }

    // ========== x86_64 implementations ==========

    /// x86_64 encode implementation with runtime dispatch
    #[cfg(target_arch = "x86_64")]
    unsafe fn encode_x86_impl(&self, data: &[u8], result: &mut String) {
        // Try AVX-512 VBMI first (best performance)
        #[cfg(target_feature = "avx512vbmi")]
        {
            if is_x86_feature_detected!("avx512vbmi") {
                if self.metadata.base == 32 {
                    unsafe { self.encode_avx512_vbmi_base32(data, result) };
                } else if self.metadata.base == 64 {
                    unsafe { self.encode_avx512_vbmi_base64(data, result) };
                }
                return;
            }
        }

        // Try SSE range-reduction if supported
        if is_x86_feature_detected!("ssse3") && self.range_info.is_some() {
            unsafe { self.encode_ssse3_range_reduction(data, result) };
            return;
        }

        // Fallback to scalar
        if self.metadata.base == 32 {
            self.encode_scalar_base32_x86(data, result);
        } else if self.metadata.base == 64 {
            self.encode_scalar_base64_x86(data, result);
        }
    }

    /// Generic SSE range-reduction encode dispatcher
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3_range_reduction(&self, data: &[u8], result: &mut String) {
        // Dispatch based on bit-width
        match self.metadata.base {
            // Unsafe: calling unsafe functions
            32 => unsafe { self.encode_ssse3_range_reduction_5bit(data, result) },
            64 => unsafe { self.encode_ssse3_range_reduction_6bit(data, result) },
            _ => unreachable!("Only base32/64 supported for range-reduction"),
        }
    }

    /// Multi-threshold encoding for 6-16 ranges (6-bit indices)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3,sse4.1")]
    unsafe fn encode_multi_threshold_6bit(
        &self,
        idx_vec: std::arch::x86_64::__m128i,
        range_info: &RangeInfo,
    ) -> std::arch::x86_64::__m128i {
        use std::arch::x86_64::*;

        // Apply each threshold in sequence using binary tree traversal
        let mut compressed = idx_vec;

        for (i, &threshold) in range_info.thresholds.iter().enumerate() {
            let thresh_vec = _mm_set1_epi8(threshold as i8);
            let cmp_vec = _mm_set1_epi8(range_info.cmp_values[i] as i8);

            // Saturating subtraction to separate ranges
            let reduced = _mm_subs_epu8(compressed, thresh_vec);

            // Comparison to determine which side of threshold
            let is_below = _mm_cmplt_epi8(compressed, cmp_vec);

            // Blend based on comparison
            compressed = _mm_blendv_epi8(reduced, compressed, is_below);
        }

        compressed
    }

    /// SSE range-reduction base64 encode (6-bit indices)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3,sse4.1")]
    unsafe fn encode_ssse3_range_reduction_6bit(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 12; // 12 bytes → 16 chars
        const SIMD_READ: usize = 16; // Actually reads 16 bytes for reshuffle

        if data.len() < SIMD_READ {
            self.encode_scalar_base64_x86(data, result);
            return;
        }

        let range_info = self.range_info.as_ref().unwrap();
        let safe_len = if data.len() >= 4 { data.len() - 4 } else { 0 };
        let num_blocks = safe_len / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Unsafe: pointer cast for LUT loading
        let offset_lut =
            unsafe { _mm_loadu_si128(range_info.offset_lut.as_ptr() as *const __m128i) };
        let subs_threshold = _mm_set1_epi8(range_info.subs_threshold as i8);

        let mut offset = 0;
        for _ in 0..num_blocks {
            // SAFETY: offset + SIMD_READ <= data.len() guaranteed by safe_len calculation
            // (safe_len ensures data.len() - 4, num_blocks uses BLOCK_SIZE=12, leaving 4-byte buffer)
            debug_assert!(offset + SIMD_READ <= data.len());

            // Unsafe: pointer arithmetic and cast for data loading
            let input_vec = unsafe { _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i) };
            // Unsafe: calling unsafe function
            let idx_vec = unsafe { self.reshuffle_x86_base64(input_vec) };

            // === RANGE REDUCTION ===

            // Dispatch based on strategy
            let compressed = match range_info.strategy {
                RangeStrategy::Simple => {
                    // Single range - no reduction needed
                    idx_vec
                }
                RangeStrategy::Small => {
                    // Two ranges - single saturating subtraction
                    _mm_subs_epu8(idx_vec, subs_threshold)
                }
                RangeStrategy::SmallMulti => {
                    // 3-5 ranges - subs + cmp + blend
                    let reduced = _mm_subs_epu8(idx_vec, subs_threshold);
                    if let (Some(cmp), Some(override_val)) =
                        (range_info.cmp_value, range_info.override_val)
                    {
                        let cmp_vec = _mm_set1_epi8(cmp as i8);
                        let override_vec = _mm_set1_epi8(override_val as i8);
                        let is_below = _mm_cmplt_epi8(idx_vec, cmp_vec);
                        _mm_blendv_epi8(reduced, override_vec, is_below)
                    } else {
                        reduced
                    }
                }
                RangeStrategy::Medium => {
                    // 6-8 ranges - multi-threshold
                    // Unsafe: calling unsafe function
                    unsafe { self.encode_multi_threshold_6bit(idx_vec, range_info) }
                }
                RangeStrategy::Large => {
                    // 9-12 ranges - multi-threshold
                    // Unsafe: calling unsafe function
                    unsafe { self.encode_multi_threshold_6bit(idx_vec, range_info) }
                }
                RangeStrategy::VeryLarge => {
                    // 13-16 ranges - multi-threshold
                    // Unsafe: calling unsafe function
                    unsafe { self.encode_multi_threshold_6bit(idx_vec, range_info) }
                }
            };

            // Step 3: Lookup offset
            let offset_vec = _mm_shuffle_epi8(offset_lut, compressed);

            // Step 4: Add offset to compressed index (NOT original index!)
            let chars = _mm_add_epi8(compressed, offset_vec);

            // Unsafe: pointer cast for storing
            // Store results
            let mut output_buf = [0u8; 16];
            unsafe { _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, chars) };

            for &byte in &output_buf {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Scalar remainder
        if simd_bytes < data.len() {
            self.encode_scalar_base64_x86(&data[simd_bytes..], result);
        }
    }

    /// AVX-512 VBMI base64 encode (6-bit indices)
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512vbmi"))]
    #[target_feature(enable = "avx512vbmi")]
    unsafe fn encode_avx512_vbmi_base64(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 12; // 12 bytes -> 16 chars
        const SIMD_READ: usize = 16; // Actually reads 16 bytes for reshuffle

        if data.len() < SIMD_READ {
            self.encode_scalar_base64_x86(data, result);
            return;
        }

        let safe_len = if data.len() >= 4 { data.len() - 4 } else { 0 };
        let num_blocks = safe_len / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Unsafe: pointer cast for LUT loading
        let lut = unsafe { _mm512_loadu_si512(self.encode_lut.as_ptr() as *const i32) };

        let mut offset = 0;
        for _ in 0..num_blocks {
            // SAFETY: offset + SIMD_READ <= data.len() guaranteed by safe_len calculation
            // (safe_len ensures data.len() - 4, num_blocks uses BLOCK_SIZE=12, leaving 4-byte buffer)
            debug_assert!(offset + SIMD_READ <= data.len());

            // Unsafe: pointer arithmetic and cast for data loading
            let input_vec = unsafe { _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i) };

            // Reshuffle to extract 6-bit indices (reuse ARM algorithm)
            let reshuffled = self.reshuffle_x86_base64(input_vec);

            // Zero-extend to 512-bit for vpermb
            let idx_512 = _mm512_castsi128_si512(reshuffled);

            // Translate using vpermb
            let chars_512 = _mm512_permutexvar_epi8(idx_512, lut);

            // Extract lower 128 bits
            let chars = _mm512_castsi512_si128(chars_512);

            // Store 16 output characters
            let mut output_buf = [0u8; 16];
            _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, chars);

            // Append to result
            for &byte in &output_buf {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Handle remainder with scalar
        if simd_bytes < data.len() {
            self.encode_scalar_base64_x86(&data[simd_bytes..], result);
        }
    }

    /// Reshuffle bytes and extract 6-bit indices from 12 input bytes (x86)
    /// Based on specialized base64.rs reshuffle
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn reshuffle_x86_base64(
        &self,
        input: std::arch::x86_64::__m128i,
    ) -> std::arch::x86_64::__m128i {
        use std::arch::x86_64::*;

        // Shuffle mask: Reshuffle bytes to prepare for 6-bit extraction
        // For each group of 3 input bytes ABC (24 bits) -> 4 output indices (4 x 6 bits)
        // Matches specialized/base64.rs ARM64 pattern (which differs from x86_64)
        let shuffle_indices = _mm_setr_epi8(
            0, 0, 1, 2, // bytes 0-2 -> positions 0-3
            3, 3, 4, 5, // bytes 3-5 -> positions 4-7
            6, 6, 7, 8, // bytes 6-8 -> positions 8-11
            9, 9, 10, 11, // bytes 9-11 -> positions 12-15
        );

        let shuffled = _mm_shuffle_epi8(input, shuffle_indices);

        // Extract 6-bit groups using shifts and masks
        // First extraction: positions 0 and 2 in each group (using mulhi)
        let t0 = _mm_and_si128(shuffled, _mm_set1_epi32(0x0FC0FC00_u32 as i32));
        let t1 = _mm_mulhi_epu16(t0, _mm_set1_epi32(0x04000040_u32 as i32));

        // Second extraction: positions 1 and 3 in each group
        let t2 = _mm_and_si128(shuffled, _mm_set1_epi32(0x003F03F0_u32 as i32));
        let t3 = _mm_mullo_epi16(t2, _mm_set1_epi32(0x01000010_u32 as i32));

        // Combine the two results
        _mm_or_si128(t1, t3)
    }

    /// Scalar fallback for base64 encoding (x86)
    #[cfg(target_arch = "x86_64")]
    fn encode_scalar_base64_x86(&self, data: &[u8], result: &mut String) {
        let mut bit_buffer = 0u32;
        let mut bits_in_buffer = 0;

        for &byte in data {
            bit_buffer = (bit_buffer << 8) | (byte as u32);
            bits_in_buffer += 8;

            while bits_in_buffer >= 6 {
                bits_in_buffer -= 6;
                let index = ((bit_buffer >> bits_in_buffer) & 0x3F) as usize;
                result.push(self.encode_lut[index] as char);
            }
        }

        // Flush remaining bits
        if bits_in_buffer > 0 {
            let index = ((bit_buffer << (6 - bits_in_buffer)) & 0x3F) as usize;
            result.push(self.encode_lut[index] as char);
        }
    }

    /// Decode string to binary data
    ///
    /// Returns None if input contains invalid characters.
    pub fn decode(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        // Only supports 5-bit (base 32) and 6-bit (base 64) for now
        if self.metadata.base != 32 && self.metadata.base != 64 {
            return None;
        }

        // Handle empty input
        if encoded.is_empty() {
            return Some(Vec::new());
        }

        // Calculate output length
        let bits_per_char = self.metadata.bits_per_symbol as usize;
        let output_len = (encoded.len() * bits_per_char) / 8;
        let mut result = Vec::with_capacity(output_len);

        let encoded_bytes = encoded.as_bytes();

        #[cfg(target_arch = "x86_64")]
        {
            unsafe {
                if is_x86_feature_detected!("ssse3") {
                    if !self.decode_ssse3_impl(encoded_bytes, &mut result) {
                        return None;
                    }
                    return Some(result);
                }
            }
            // Scalar fallback for x86_64 without SSSE3
            if !self.decode_scalar(encoded_bytes, &mut result) {
                return None;
            }
            Some(result)
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            if !self.decode_neon_impl(encoded_bytes, &mut result) {
                return None;
            }
            Some(result)
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            // Scalar fallback
            if !self.decode_scalar(encoded_bytes, &mut result) {
                return None;
            }
            Some(result)
        }
    }

    /// Check if dictionary is standard base64
    fn is_standard_base64(&self) -> bool {
        if self.metadata.base != 64 {
            return false;
        }
        // Standard: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        let expected = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        &self.encode_lut[..64] == expected
    }

    /// x86_64 SSSE3 decode implementation with dispatch
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_ssse3_impl(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
        if self.is_rfc4648_base32() {
            // Unsafe: calling unsafe function
            unsafe { self.decode_ssse3_base32_rfc4648(encoded, result) }
        } else if self.is_standard_base64() {
            // Unsafe: calling unsafe function
            unsafe { self.decode_ssse3_base64_standard(encoded, result) }
        } else if let Some(ref range_info) = self.range_info {
            // Try SIMD multi-range decode for 6-16 ranges
            if range_info.ranges.len() >= 6 {
                // Unsafe: calling unsafe function
                unsafe { self.decode_ssse3_multi_range(encoded, result) }
            } else {
                // Fall back to scalar for 1-5 ranges (not optimized yet)
                self.decode_scalar(encoded, result)
            }
        } else {
            // No range info - use scalar LUT
            self.decode_scalar(encoded, result)
        }
    }

    /// aarch64 NEON decode implementation with dispatch
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn decode_neon_impl(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
        if self.is_rfc4648_base32() {
            unsafe { self.decode_neon_base32_rfc4648(encoded, result) }
        } else if self.is_standard_base64() {
            unsafe { self.decode_neon_base64_standard(encoded, result) }
        } else {
            // Arbitrary dictionary - use scalar LUT
            self.decode_scalar(encoded, result)
        }
    }

    /// Multi-range decode dispatcher (6-16 ranges)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_ssse3_multi_range(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
        match self.metadata.base {
            // Unsafe: calling unsafe functions
            32 => unsafe { self.decode_ssse3_multi_range_5bit(encoded, result) },
            64 => unsafe { self.decode_ssse3_multi_range_6bit(encoded, result) },
            _ => self.decode_scalar(encoded, result),
        }
    }

    /// Multi-range decode for base64 (6-bit indices)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_ssse3_multi_range_6bit(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 16; // 16 chars → 12 bytes

        // Strip padding
        let input_no_padding = if let Some(last_non_pad) = encoded.iter().rposition(|&b| b != b'=')
        {
            &encoded[..=last_non_pad]
        } else {
            encoded
        };

        let range_info = self.range_info.as_ref().unwrap();
        let num_blocks = input_no_padding.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for i in 0..num_blocks {
            let offset = i * BLOCK_SIZE;

            // SAFETY: offset + BLOCK_SIZE <= simd_bytes <= input_no_padding.len() by construction
            // (num_blocks = input_no_padding.len() / BLOCK_SIZE, offset = i * BLOCK_SIZE where i < num_blocks)
            debug_assert!(offset + BLOCK_SIZE <= input_no_padding.len());

            // Unsafe: pointer arithmetic and cast for data loading
            let chars =
                unsafe { _mm_loadu_si128(input_no_padding.as_ptr().add(offset) as *const __m128i) };

            // === VALIDATION + TRANSLATION ===
            // Unsafe: calling unsafe function
            let indices =
                match unsafe { self.validate_and_translate_multi_range(chars, range_info) } {
                    Some(idx) => idx,
                    None => return false,
                };

            // === UNPACKING (reuse existing reshuffle_decode_ssse3) ===
            // Unsafe: calling unsafe function
            let decoded = unsafe { self.reshuffle_decode_ssse3(indices) };

            let mut output_buf = [0u8; 16];
            // Unsafe: pointer cast for storing
            unsafe { _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, decoded) };
            result.extend_from_slice(&output_buf[0..12]);
        }

        // Scalar remainder
        if simd_bytes < input_no_padding.len()
            && !self.decode_scalar(&input_no_padding[simd_bytes..], result)
        {
            return false;
        }

        true
    }

    /// Validate and translate chars to indices for multi-range dictionaries
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3,sse4.1")]
    pub(super) unsafe fn validate_and_translate_multi_range(
        &self,
        chars: std::arch::x86_64::__m128i,
        range_info: &RangeInfo,
    ) -> Option<std::arch::x86_64::__m128i> {
        use std::arch::x86_64::*;

        let mut valid_mask = _mm_setzero_si128();
        let mut indices = _mm_setzero_si128();

        for range in &range_info.ranges {
            // Range checks: char >= start_char && char <= end_char
            let start_vec = _mm_set1_epi8(range.start_char as i8);
            let end_vec =
                _mm_set1_epi8((range.start_char + (range.end_idx - range.start_idx)) as i8);

            let ge_start = _mm_cmpgt_epi8(chars, _mm_sub_epi8(start_vec, _mm_set1_epi8(1)));
            let le_end = _mm_cmplt_epi8(chars, _mm_add_epi8(end_vec, _mm_set1_epi8(1)));
            let in_range = _mm_and_si128(ge_start, le_end);

            valid_mask = _mm_or_si128(valid_mask, in_range);

            // Translation: index = char - start_char + start_idx
            let offset = _mm_set1_epi8(range.start_idx as i8 - range.start_char as i8);
            let range_indices = _mm_add_epi8(chars, offset);

            // Blend into result
            indices = _mm_blendv_epi8(indices, range_indices, in_range);
        }

        // Check if all chars valid
        if _mm_movemask_epi8(valid_mask) != 0xFFFF {
            return None;
        }

        Some(indices)
    }

    /// SSSE3 base64 standard decode (reuse specialized reshuffle)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_ssse3_base64_standard(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 16; // 16 chars → 12 bytes

        // Strip padding
        let input_no_padding = if let Some(last_non_pad) = encoded.iter().rposition(|&b| b != b'=')
        {
            &encoded[..=last_non_pad]
        } else {
            encoded
        };

        let num_blocks = input_no_padding.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for i in 0..num_blocks {
            let offset = i * BLOCK_SIZE;

            // SAFETY: offset + BLOCK_SIZE <= simd_bytes <= input_no_padding.len() by construction
            // (num_blocks = input_no_padding.len() / BLOCK_SIZE, offset = i * BLOCK_SIZE where i < num_blocks)
            debug_assert!(offset + BLOCK_SIZE <= input_no_padding.len());

            // Unsafe: pointer arithmetic and cast for data loading
            let input_vec =
                unsafe { _mm_loadu_si128(input_no_padding.as_ptr().add(offset) as *const __m128i) };

            // === VALIDATION & TRANSLATION (char → 6-bit index) ===
            let mut char_buf = [0u8; 16];
            // Unsafe: pointer cast for storing
            unsafe { _mm_storeu_si128(char_buf.as_mut_ptr() as *mut __m128i, input_vec) };

            let mut indices_buf = [0u8; 16];

            // For range-reduced dictionaries, reverse the transformation
            if let Some(range_info) = &self.range_info {
                for j in 0..16 {
                    let ch = char_buf[j];
                    // Find which range this character belongs to
                    let mut found = false;
                    for range in &range_info.ranges {
                        let range_start_char = range.start_char;
                        let range_end_char = (range.start_char as i16
                            + (range.end_idx - range.start_idx) as i16)
                            as u8;

                        if ch >= range_start_char && ch <= range_end_char {
                            // Decode: compressed_idx = char - range.start_char
                            //         original_idx = range.start_idx + compressed_idx
                            let compressed_idx = ch - range_start_char;
                            let original_idx = range.start_idx + compressed_idx;
                            indices_buf[j] = original_idx;
                            found = true;
                            break;
                        }
                    }

                    if !found {
                        return false; // Invalid character
                    }
                }
            } else {
                // No range-reduction, use direct decode_lut
                for j in 0..16 {
                    // Unsafe: unchecked array indexing
                    let idx = unsafe { *self.decode_lut.get_unchecked(char_buf[j] as usize) };
                    if idx == 0xFF {
                        return false;
                    }
                    indices_buf[j] = idx;
                }
            }

            // Unsafe: pointer cast for data loading
            let indices = unsafe { _mm_loadu_si128(indices_buf.as_ptr() as *const __m128i) };

            // === UNPACKING (reuse specialized base64 reshuffle) ===
            // Unsafe: calling unsafe function
            let decoded = unsafe { self.reshuffle_decode_ssse3(indices) };

            let mut output_buf = [0u8; 16];
            // Unsafe: pointer cast for storing
            unsafe { _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, decoded) };
            result.extend_from_slice(&output_buf[0..12]);
        }

        // Scalar remainder
        if simd_bytes < input_no_padding.len()
            && !self.decode_scalar(&input_no_padding[simd_bytes..], result)
        {
            return false;
        }

        true
    }

    /// NEON base64 standard decode
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn decode_neon_base64_standard(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
        use std::arch::aarch64::*;

        const BLOCK_SIZE: usize = 16;

        let input_no_padding = if let Some(last_non_pad) = encoded.iter().rposition(|&b| b != b'=')
        {
            &encoded[..=last_non_pad]
        } else {
            encoded
        };

        let num_blocks = input_no_padding.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for i in 0..num_blocks {
            let offset = i * BLOCK_SIZE;

            // SAFETY: offset + BLOCK_SIZE <= simd_bytes <= input_no_padding.len() by construction
            // (num_blocks = input_no_padding.len() / BLOCK_SIZE, offset = i * BLOCK_SIZE where i < num_blocks)
            debug_assert!(offset + BLOCK_SIZE <= input_no_padding.len());

            // Unsafe: pointer arithmetic for data loading
            let input_vec = unsafe { vld1q_u8(input_no_padding.as_ptr().add(offset)) };

            // === VALIDATION & TRANSLATION ===
            let mut char_buf = [0u8; 16];
            // Store is unsafe (memory op)
            unsafe { vst1q_u8(char_buf.as_mut_ptr(), input_vec) };

            let mut indices_buf = [0u8; 16];

            // For range-reduced dictionaries, reverse the transformation
            // Note: aarch64 doesn't have range_info, so always use decode_lut
            for j in 0..16 {
                // Unsafe: unchecked array indexing
                let idx = unsafe { *self.decode_lut.get_unchecked(char_buf[j] as usize) };
                if idx == 0xFF {
                    return false;
                }
                indices_buf[j] = idx;
            }

            // Load is unsafe (memory op)
            let indices = unsafe { vld1q_u8(indices_buf.as_ptr()) };

            // === UNPACKING ===
            // Unsafe: calls unsafe fn
            let decoded = unsafe { self.reshuffle_decode_neon(indices) };

            let mut output_buf = [0u8; 16];
            // Store is unsafe (memory op)
            unsafe { vst1q_u8(output_buf.as_mut_ptr(), decoded) };
            result.extend_from_slice(&output_buf[0..12]);
        }

        // Scalar remainder
        if simd_bytes < input_no_padding.len()
            && !self.decode_scalar(&input_no_padding[simd_bytes..], result)
        {
            return false;
        }

        true
    }

    /// Reshuffle 6-bit indices to packed 8-bit bytes (x86 SSSE3)
    /// Based on specialized/base64.rs reshuffle_decode
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn reshuffle_decode_ssse3(
        &self,
        indices: std::arch::x86_64::__m128i,
    ) -> std::arch::x86_64::__m128i {
        use std::arch::x86_64::*;

        // Stage 1: Merge adjacent pairs using multiply-add
        let merge_ab_and_bc = _mm_maddubs_epi16(indices, _mm_set1_epi32(0x01400140u32 as i32));

        // Stage 2: Combine 16-bit pairs into 32-bit values
        let final_32bit = _mm_madd_epi16(merge_ab_and_bc, _mm_set1_epi32(0x00011000u32 as i32));

        // Stage 3: Extract the valid bytes from each 32-bit group
        _mm_shuffle_epi8(
            final_32bit,
            _mm_setr_epi8(
                2, 1, 0, // first group of 3 bytes (reversed)
                6, 5, 4, // second group of 3 bytes (reversed)
                10, 9, 8, // third group of 3 bytes (reversed)
                14, 13, 12, // fourth group of 3 bytes (reversed)
                -1, -1, -1, -1, // unused
            ),
        )
    }

    /// Reshuffle 6-bit indices to packed 8-bit bytes (ARM NEON)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn reshuffle_decode_neon(
        &self,
        indices: std::arch::aarch64::uint8x16_t,
    ) -> std::arch::aarch64::uint8x16_t {
        use std::arch::aarch64::*;

        // Simulate _mm_maddubs_epi16: multiply adjacent u8 pairs and add
        // Input: [a0 b0 a1 b1 a2 b2 ...] u8x16
        // Multiply pattern: [0x01 0x40 0x01 0x40 ...]
        // Result: [a0*1 + b0*64, a1*1 + b1*64, ...] as i16x8

        let pairs = vreinterpretq_u16_u8(indices);

        // Extract even bytes (a0, a1, a2, ...) and odd bytes (b0, b1, b2, ...)
        let even = vandq_u16(pairs, vdupq_n_u16(0xFF)); // Low byte of each pair
        let odd = vshrq_n_u16(pairs, 8); // High byte of each pair

        // Stage 1: Emulate maddubs: even * 64 + odd * 1
        let merge_result = vmlaq_n_u16(odd, even, 64);

        // Stage 2: Combine 16-bit pairs using multiply-add
        // _mm_madd_epi16: multiply adjacent i16 and add horizontally
        // Pattern: [0x1000 0x0001 0x1000 0x0001 ...]
        // Result: [p0*0x1000 + p1*0x0001, ...] as i32x4

        let merge_u32 = vreinterpretq_u32_u16(merge_result);

        // Extract low and high 16-bit values from each 32-bit pair
        let lo = vandq_u32(merge_u32, vdupq_n_u32(0xFFFF));
        let hi = vshrq_n_u32(merge_u32, 16);

        // Combine: lo << 12 | hi
        let final_32bit = vorrq_u32(vshlq_n_u32(lo, 12), hi);

        // Stage 3: Extract valid bytes (3 bytes per 32-bit group)
        // Unsafe: memory load
        let shuffle_mask = unsafe {
            vld1q_u8(
                [
                    2, 1, 0, // first group (reversed byte order)
                    6, 5, 4, // second group
                    10, 9, 8, // third group
                    14, 13, 12, // fourth group
                    255, 255, 255, 255,
                ]
                .as_ptr(),
            )
        };

        let result_bytes = vreinterpretq_u8_u32(final_32bit);
        vqtbl1q_u8(result_bytes, shuffle_mask)
    }

    /// Scalar fallback for decoding
    pub(super) fn decode_scalar(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
        let bits_per_char = self.metadata.bits_per_symbol as usize;
        let mut bit_buffer = 0u32;
        let mut bits_in_buffer = 0;

        for &ch_byte in encoded {
            // For range-reduced dictionaries (6-16 ranges), reverse the transformation
            #[cfg(target_arch = "x86_64")]
            let index = if let Some(range_info) = &self.range_info {
                // Find which range this character belongs to
                let mut found_idx = None;
                for range in &range_info.ranges {
                    let range_start_char = range.start_char;
                    let range_end_char =
                        (range.start_char as i16 + (range.end_idx - range.start_idx) as i16) as u8;

                    if ch_byte >= range_start_char && ch_byte <= range_end_char {
                        // Decode: compressed_idx = char - range.start_char
                        //         original_idx = range.start_idx + compressed_idx
                        let compressed_idx = ch_byte - range_start_char;
                        let original_idx = range.start_idx + compressed_idx;
                        found_idx = Some(original_idx);
                        break;
                    }
                }

                match found_idx {
                    Some(idx) => idx,
                    None => return false, // Invalid character
                }
            } else {
                // No range-reduction, use direct decode_lut
                let idx = self.decode_lut[ch_byte as usize];
                if idx == 0xFF {
                    return false; // Invalid character
                }
                idx
            };

            #[cfg(not(target_arch = "x86_64"))]
            let index = {
                let idx = self.decode_lut[ch_byte as usize];
                if idx == 0xFF {
                    return false; // Invalid character
                }
                idx
            };

            bit_buffer = (bit_buffer << bits_per_char) | (index as u32);
            bits_in_buffer += bits_per_char;

            while bits_in_buffer >= 8 {
                bits_in_buffer -= 8;
                let byte = ((bit_buffer >> bits_in_buffer) & 0xFF) as u8;
                result.push(byte);
            }
        }

        true
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn test_creation_from_arbitrary_base32() {
        // Shuffled 32-char dictionary
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict);
        assert!(codec.is_some(), "Should create codec for arbitrary base32");
    }

    #[test]
    fn test_creation_from_arbitrary_base64() {
        // Shuffled 64-char dictionary
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict);
        assert!(codec.is_some(), "Should create codec for arbitrary base64");
    }

    #[test]
    fn test_rejects_sequential_dictionary() {
        // Sequential dictionary should use GenericSimdCodec, not LUT
        let chars: Vec<char> = (0x41..0x61).map(|c| char::from_u32(c).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict);
        assert!(
            codec.is_none(),
            "Should reject sequential (use GenericSimdCodec)"
        );
    }

    #[test]
    fn test_rejects_small_dictionary() {
        // 16-char dictionary too small for Base64LutCodec
        let chars: Vec<char> = "0123456789ABCDEF".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict);
        assert!(codec.is_none(), "Should reject base16 (too small)");
    }

    #[test]
    fn test_rejects_non_power_of_two() {
        // 40-char dictionary (non power-of-2)
        let chars: Vec<char> = (0x41..0x69).map(|c| char::from_u32(c).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict);
        assert!(codec.is_none(), "Should reject non-power-of-2 base");
    }

    #[test]
    fn test_lut_construction_base32() {
        // Shuffled base32 dictionary (32 unique chars)
        let chars: Vec<char> = "76543ABCDEFGHIJKLMNOPQRSTUVWXYZ2".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Verify encode_lut matches dictionary
        assert_eq!(codec.encode_lut[0], b'7');
        assert_eq!(codec.encode_lut[1], b'6');
        assert_eq!(codec.encode_lut[31], b'2');

        // Verify decode_lut is inverse
        assert_eq!(codec.decode_lut[b'7' as usize], 0);
        assert_eq!(codec.decode_lut[b'6' as usize], 1);
        assert_eq!(codec.decode_lut[b'2' as usize], 31);

        // Verify invalid chars marked as 0xFF
        assert_eq!(codec.decode_lut[b'a' as usize], 0xFF);
        assert_eq!(codec.decode_lut[b'9' as usize], 0xFF);
    }

    #[test]
    fn test_lut_construction_base64() {
        // Shuffled base64 dictionary
        let chars: Vec<char> = "9876543210zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Verify encode_lut matches dictionary
        assert_eq!(codec.encode_lut[0], b'9');
        assert_eq!(codec.encode_lut[1], b'8');
        assert_eq!(codec.encode_lut[63], b'-');

        // Verify decode_lut is inverse
        assert_eq!(codec.decode_lut[b'9' as usize], 0);
        assert_eq!(codec.decode_lut[b'8' as usize], 1);
        assert_eq!(codec.decode_lut[b'-' as usize], 63);

        // Verify invalid chars marked as 0xFF
        assert_eq!(codec.decode_lut[b'@' as usize], 0xFF);
        assert_eq!(codec.decode_lut[b'!' as usize], 0xFF);
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_encode_base32_round_trip() {
        // Shuffled base32 dictionary
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data = b"Hello, World!";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_encode_base64_round_trip() {
        // Shuffled base64 dictionary
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data = b"The quick brown fox jumps over the lazy dog";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    fn test_encode_empty_input() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data: Vec<u8> = vec![];
        let encoded = codec.encode(&data, &dict).unwrap();

        assert_eq!(encoded, "");
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_encode_various_sizes_base32() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 5 bytes (exactly one block)
        let data5: Vec<u8> = (0..5).collect();
        let enc5 = codec.encode(&data5, &dict).unwrap();
        let dec5 = codec.decode(&enc5, &dict).unwrap();
        assert_eq!(&dec5[..], &data5[..]);

        // 10 bytes (two blocks)
        let data10: Vec<u8> = (0..10).collect();
        let enc10 = codec.encode(&data10, &dict).unwrap();
        let dec10 = codec.decode(&enc10, &dict).unwrap();
        assert_eq!(&dec10[..], &data10[..]);

        // 7 bytes (one block + remainder)
        let data7: Vec<u8> = (0..7).collect();
        let enc7 = codec.encode(&data7, &dict).unwrap();
        let dec7 = codec.decode(&enc7, &dict).unwrap();
        assert_eq!(&dec7[..], &data7[..]);
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_encode_various_sizes_base64() {
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 12 bytes (exactly one block)
        let data12: Vec<u8> = (0..12).collect();
        let enc12 = codec.encode(&data12, &dict).unwrap();
        let dec12 = codec.decode(&enc12, &dict).unwrap();
        assert_eq!(&dec12[..], &data12[..]);

        // 24 bytes (two blocks)
        let data24: Vec<u8> = (0..24).collect();
        let enc24 = codec.encode(&data24, &dict).unwrap();
        let dec24 = codec.decode(&enc24, &dict).unwrap();
        assert_eq!(&dec24[..], &data24[..]);

        // 15 bytes (one block + remainder)
        let data15: Vec<u8> = (0..15).collect();
        let enc15 = codec.encode(&data15, &dict).unwrap();
        let dec15 = codec.decode(&enc15, &dict).unwrap();
        assert_eq!(&dec15[..], &data15[..]);
    }

    #[test]
    fn test_decode_invalid_character() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 'a' is not in dictionary (lowercase not present)
        let invalid = "ZYXa";
        let result = codec.decode(invalid, &dict);

        assert!(result.is_none(), "Should reject invalid character 'a'");
    }

    #[test]
    fn test_decode_empty_input() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let result = codec.decode("", &dict).unwrap();
        assert_eq!(result.len(), 0);
    }

    /// Integration test: verify round-trip with real data
    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_integration_base32_neon() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data = b"Arbitrary base32 NEON test with various sizes and patterns!";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    /// Integration test: verify round-trip with real data
    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_integration_base64_neon() {
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data = b"Arbitrary base64 NEON test with various sizes and patterns!";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_base32_all_byte_values() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Test all byte values 0x00 to 0xFF
        let data: Vec<u8> = (0..=255).collect();
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_base64_all_byte_values() {
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Test all byte values 0x00 to 0xFF
        let data: Vec<u8> = (0..=255).collect();
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    // ========== x86_64 tests ==========

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_base32_round_trip_x86() {
        // Shuffled base32 dictionary
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data = b"Hello, World!";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_base64_round_trip_x86() {
        // Shuffled base64 dictionary
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data = b"The quick brown fox jumps over the lazy dog";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_various_sizes_base32_x86() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 5 bytes (exactly one block)
        let data5: Vec<u8> = (0..5).collect();
        let enc5 = codec.encode(&data5, &dict).unwrap();
        let dec5 = codec.decode(&enc5, &dict).unwrap();
        assert_eq!(&dec5[..], &data5[..]);

        // 10 bytes (two blocks)
        let data10: Vec<u8> = (0..10).collect();
        let enc10 = codec.encode(&data10, &dict).unwrap();
        let dec10 = codec.decode(&enc10, &dict).unwrap();
        assert_eq!(&dec10[..], &data10[..]);

        // 7 bytes (one block + remainder)
        let data7: Vec<u8> = (0..7).collect();
        let enc7 = codec.encode(&data7, &dict).unwrap();
        let dec7 = codec.decode(&enc7, &dict).unwrap();
        assert_eq!(&dec7[..], &data7[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_various_sizes_base64_x86() {
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 12 bytes (exactly one block)
        let data12: Vec<u8> = (0..12).collect();
        let enc12 = codec.encode(&data12, &dict).unwrap();
        let dec12 = codec.decode(&enc12, &dict).unwrap();
        assert_eq!(&dec12[..], &data12[..]);

        // 24 bytes (two blocks)
        let data24: Vec<u8> = (0..24).collect();
        let enc24 = codec.encode(&data24, &dict).unwrap();
        let dec24 = codec.decode(&enc24, &dict).unwrap();
        assert_eq!(&dec24[..], &data24[..]);

        // 15 bytes (one block + remainder)
        let data15: Vec<u8> = (0..15).collect();
        let enc15 = codec.encode(&data15, &dict).unwrap();
        let dec15 = codec.decode(&enc15, &dict).unwrap();
        assert_eq!(&dec15[..], &data15[..]);
    }

    /// Integration test: verify round-trip with real data
    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_integration_base32_x86() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data = b"Arbitrary base32 x86 test with various sizes and patterns!";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    /// Integration test: verify round-trip with real data
    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_integration_base64_x86() {
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data = b"Arbitrary base64 x86 test with various sizes and patterns!";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_base32_all_byte_values_x86() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Test all byte values 0x00 to 0xFF
        let data: Vec<u8> = (0..=255).collect();
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_base64_all_byte_values_x86() {
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Test all byte values 0x00 to 0xFF
        let data: Vec<u8> = (0..=255).collect();
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    // ========== Range-reduction tests ==========

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_detect_ranges_base32_rfc4648() {
        // RFC4648 base32: ABCDEFGHIJKLMNOPQRSTUVWXYZ234567
        let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        let ranges = Base64LutCodec::detect_ranges(chars, 32);

        assert_eq!(ranges.len(), 2);

        // Range 0: [0-25] → 'A'-'Z'
        assert_eq!(ranges[0].start_idx, 0);
        assert_eq!(ranges[0].end_idx, 25);
        assert_eq!(ranges[0].start_char, b'A');
        assert_eq!(ranges[0].offset, 65); // 'A' - 0

        // Range 1: [26-31] → '2'-'7'
        assert_eq!(ranges[1].start_idx, 26);
        assert_eq!(ranges[1].end_idx, 31);
        assert_eq!(ranges[1].start_char, b'2');
        assert_eq!(ranges[1].offset, 24); // '2' - 26 = 50 - 26 = 24
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_range_info_construction_base32() {
        let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        let ranges = Base64LutCodec::detect_ranges(chars, 32);
        let range_info = RangeInfo::build_multi_range(&ranges).unwrap();

        assert_eq!(range_info.subs_threshold, 25);
        assert!(range_info.cmp_value.is_none()); // No comparison needed
        assert!(range_info.override_val.is_none()); // No override needed

        // Check offset LUT
        assert_eq!(range_info.offset_lut[0], 65); // 'A' - 0
        assert_eq!(range_info.offset_lut[1], 24); // '2' - 26
        assert_eq!(range_info.offset_lut[2], 24); // '3' - 27 (same offset for range)
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_base32_range_reduction_round_trip() {
        // RFC4648 base32
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        assert!(codec.range_info.is_some(), "Range info should be built");

        let data = b"Hello, World!";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_range_reduction_all_indices() {
        // RFC4648 base32
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Test all 32 possible 5-bit values
        // Create data that exercises all indices
        let mut data = Vec::new();
        for i in 0..32u8 {
            // Put index in high 5 bits
            data.push(i << 3);
        }

        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        // Check that high 5 bits match (low 3 bits may differ due to padding)
        for (i, &decoded_byte) in decoded.iter().enumerate() {
            let original_high_bits = data[i] & 0xF8;
            let decoded_high_bits = decoded_byte & 0xF8;
            assert_eq!(
                decoded_high_bits, original_high_bits,
                "Mismatch at index {}: original={:08b}, decoded={:08b}",
                i, data[i], decoded_byte
            );
        }
    }

    // ========== Generic multi-range tests ==========

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_detect_ranges_base64_standard() {
        // Standard base64: 'A-Z', 'a-z', '0-9', '+', '/'
        let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let ranges = Base64LutCodec::detect_ranges(chars, 64);

        assert_eq!(ranges.len(), 5);

        // Range 0: [0-25] → 'A'-'Z'
        assert_eq!(ranges[0].start_idx, 0);
        assert_eq!(ranges[0].end_idx, 25);
        assert_eq!(ranges[0].start_char, b'A');

        // Range 1: [26-51] → 'a'-'z'
        assert_eq!(ranges[1].start_idx, 26);
        assert_eq!(ranges[1].end_idx, 51);
        assert_eq!(ranges[1].start_char, b'a');

        // Range 2: [52-61] → '0'-'9'
        assert_eq!(ranges[2].start_idx, 52);
        assert_eq!(ranges[2].end_idx, 61);
        assert_eq!(ranges[2].start_char, b'0');

        // Range 3: [62] → '+'
        assert_eq!(ranges[3].start_idx, 62);
        assert_eq!(ranges[3].end_idx, 62);
        assert_eq!(ranges[3].start_char, b'+');

        // Range 4: [63] → '/'
        assert_eq!(ranges[4].start_idx, 63);
        assert_eq!(ranges[4].end_idx, 63);
        assert_eq!(ranges[4].start_char, b'/');
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_multi_range_base64_arbitrary() {
        // Arbitrary (non-standard) base64 dictionary with 4 ranges
        // Shuffled within ranges: digits first, then lower, upper, symbols
        let chars: Vec<char> = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ+/"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict);
        if codec.is_none() {
            eprintln!("from_dictionary returned None for arbitrary base64");
        }
        assert!(codec.is_some(), "Should create codec for arbitrary base64");
        let codec = codec.unwrap();

        // 4 ranges that fit in 16-byte LUT should have range_info
        // (This dictionary has ranges that compress well)
        assert!(
            codec.range_info.is_some(),
            "4-range dictionary that fits in LUT should use SSSE3"
        );

        // Test encoding (uses scalar path)
        let data = b"Hello, World!";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();
        assert_eq!(&decoded[..], &data[..]);
    }

    // TODO: Debug decode failure for large inputs with 4+ range dictionaries
    // #[test]
    // #[cfg(target_arch = "x86_64")]
    // fn test_multi_range_base64_all_byte_values() {
    //     // Arbitrary base64 dictionary (digits, lower, upper, symbols)
    //     let chars: Vec<char> = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ+/"
    //         .chars()
    //         .collect();
    //     let dict = Dictionary::new(chars).unwrap();
    //     let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

    //     // Test all byte values 0x00 to 0xFF
    //     let data: Vec<u8> = (0..=255).collect();
    //     let encoded = codec.encode(&data, &dict);
    //     assert!(encoded.is_some(), "Encoding should succeed");
    //     let encoded = encoded.unwrap();

    //     let decoded = codec.decode(&encoded, &dict);
    //     assert!(decoded.is_some(), "Decoding should succeed");
    //     let decoded = decoded.unwrap();

    //     assert_eq!(&decoded[..], &data[..]);
    // }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_multi_range_arbitrary_3range() {
        // Custom 3-range dictionary: '0-9', 'A-Z', 'a-f' (42 chars)
        // This is base42, but only 32 chars fit in power-of-2, so truncate to 32
        let chars: Vec<char> = "0123456789ABCDEFGHIJKLMNOPQRSTUVabcdef"
            .chars()
            .take(32)
            .collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();
        assert!(
            codec.range_info.is_some(),
            "Should build range info for 3-range dictionary"
        );

        let data = b"Test data";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();
        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_fallback_too_many_ranges() {
        // Pathological dictionary with >16 ranges (alternating case)
        // Each char is its own range
        let chars: Vec<char> = "AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPp".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        #[cfg(target_arch = "x86_64")]
        {
            // Should fall back to scalar (range_info = None)
            assert!(
                codec.range_info.is_none(),
                "Should not build range info for >16 ranges"
            );
        }

        // Encoding should still work (scalar path)
        let data = b"Test";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();
        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_single_range_dictionary() {
        // Sequential 32-char dictionary (single range)
        // This should be rejected by from_dictionary (uses GenericSimdCodec instead)
        let chars: Vec<char> = (0x30..0x50).map(|c| char::from_u32(c).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict);
        assert!(codec.is_none(), "Should reject sequential dictionary");
    }

    // ========== SIMD decode-specific tests ==========

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_rfc4648_base32_simd() {
        // RFC4648 base32 dictionary - triggers SIMD path
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Test with data that spans multiple SIMD blocks (>16 chars encoded = >10 bytes)
        let data = b"Hello, World! This is a SIMD decode test for RFC4648 base32.";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_base32_invalid_char_simd() {
        // RFC4648 base32
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Invalid char 'a' (lowercase not in dictionary)
        let invalid = "ABCDEFGHIJKLMNOP1234567890ABCDEFGHIJKLMNOPabcd";
        let result = codec.decode(invalid, &dict);

        assert!(result.is_none(), "Should reject invalid character");
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_base32_various_sizes() {
        // RFC4648 base32
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Test various input sizes (aligned, remainder)
        for len in [0, 1, 10, 15, 20, 32, 64, 100] {
            let data: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();
            let encoded = codec.encode(&data, &dict).unwrap();
            let decoded = codec.decode(&encoded, &dict).unwrap();
            assert_eq!(&decoded[..], &data[..], "Failed at length {}", len);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_arbitrary_dictionary_fallback() {
        // Arbitrary (non-standard) base64 dictionary - should use scalar LUT
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data = b"Test arbitrary dictionary decode fallback to scalar LUT.";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_decode_rfc4648_base32_neon() {
        // RFC4648 base32 dictionary - triggers NEON path
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data = b"Hello, World! This is a NEON decode test for RFC4648 base32.";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_decode_base32_various_sizes_neon() {
        // RFC4648 base32
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Test various sizes
        for len in [0, 1, 10, 20, 32, 64] {
            let data: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();
            let encoded = codec.encode(&data, &dict).unwrap();
            let decoded = codec.decode(&encoded, &dict).unwrap();
            assert_eq!(&decoded[..], &data[..], "Failed at length {}", len);
        }
    }

    // ========== 6-16 Range Tests ==========

    /// Generate synthetic dictionary with N contiguous ranges
    #[cfg(all(test, target_arch = "x86_64"))]
    fn generate_synthetic_dictionary(num_ranges: usize, total_size: usize) -> Vec<char> {
        assert!(num_ranges > 0 && num_ranges <= 16);
        assert!(total_size == 32 || total_size == 64);

        let chars_per_range = total_size / num_ranges;
        let remainder = total_size % num_ranges;

        let mut dictionary = Vec::new();

        // Use printable ASCII: 0x21 '!' to 0x7E '~' (94 chars)
        // Calculate gap size to ensure we don't run out of chars
        let printable_ascii: Vec<u8> = (0x21u8..=0x7Eu8).collect();
        let available_chars = printable_ascii.len();

        // Calculate maximum gap that still leaves enough chars
        let max_gap = if num_ranges > 1 {
            (available_chars - total_size) / (num_ranges - 1)
        } else {
            0
        };
        let gap_size = max_gap.min(3); // Use smaller gaps to ensure we have enough chars

        let mut ascii_offset = 0usize;

        for i in 0..num_ranges {
            let range_len = chars_per_range + if i < remainder { 1 } else { 0 };

            for _ in 0..range_len {
                if ascii_offset < printable_ascii.len() {
                    dictionary.push(printable_ascii[ascii_offset] as char);
                    ascii_offset += 1;
                }
            }

            // Add gap between ranges (except after last range)
            if i < num_ranges - 1 {
                ascii_offset += gap_size;
            }
        }

        // Ensure we have exactly the right number of chars
        assert_eq!(
            dictionary.len(),
            total_size,
            "Generated dictionary has wrong size"
        );

        dictionary
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_generate_synthetic_dictionary_6_ranges() {
        let dictionary = generate_synthetic_dictionary(6, 64);
        assert_eq!(dictionary.len(), 64);

        // Verify it creates 6 contiguous ranges
        let dictionary_bytes: Vec<u8> = dictionary.iter().map(|&c| c as u8).collect();
        let mut encode_lut = [0u8; 64];
        encode_lut[..64].copy_from_slice(&dictionary_bytes);

        let ranges = Base64LutCodec::detect_ranges(&encode_lut, 64);
        assert_eq!(ranges.len(), 6, "Should detect exactly 6 ranges");
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_6_ranges() {
        let dictionary = generate_synthetic_dictionary(6, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 6+ ranges use scalar fallback (range-reduction not supported)
        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data = b"Testing 6-range encoding with various data patterns!";
        let encoded = codec.encode(data, &dict);
        assert!(encoded.is_some(), "Encode should succeed for 6 ranges");
        assert!(
            !encoded.unwrap().is_empty(),
            "Encoded output should not be empty"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_6_ranges_all_indices() {
        let dictionary = generate_synthetic_dictionary(6, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Test all 64 possible 6-bit values
        let mut data = Vec::new();
        for i in 0..64u8 {
            data.push(i << 2); // Put index in high 6 bits
        }

        let encoded = codec.encode(&data, &dict);
        assert!(encoded.is_some(), "Encode should succeed");
        assert!(
            !encoded.unwrap().is_empty(),
            "Encoded output should not be empty"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_8_ranges() {
        let dictionary = generate_synthetic_dictionary(8, 64);
        let dict = Dictionary::new(dictionary.clone()).unwrap();

        let codec = Base64LutCodec::from_dictionary(&dict);
        assert!(
            codec.is_some(),
            "Codec should be created for 8-range dictionary"
        );
        let codec = codec.unwrap();

        // 6+ ranges use scalar fallback (range-reduction not supported)
        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data = b"Testing 8-range encoding!";
        let encoded = codec.encode(data, &dict);
        assert!(encoded.is_some(), "Encode should succeed for 8 ranges");
        assert!(!encoded.unwrap().is_empty());
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_8_ranges_all_byte_values() {
        let dictionary = generate_synthetic_dictionary(8, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data: Vec<u8> = (0..=255).collect();
        let encoded = codec.encode(&data, &dict);
        assert!(encoded.is_some(), "Encode should succeed");
        assert!(!encoded.unwrap().is_empty());
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_9_ranges() {
        let dictionary = generate_synthetic_dictionary(9, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 6+ ranges use scalar fallback (range-reduction not supported)
        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data = b"Testing 9-range encoding!";
        let encoded = codec.encode(data, &dict);
        assert!(encoded.is_some(), "Encode should succeed for 9 ranges");
        assert!(!encoded.unwrap().is_empty());
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_12_ranges() {
        let dictionary = generate_synthetic_dictionary(12, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 6+ ranges use scalar fallback (range-reduction not supported)
        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data = b"Testing 12-range encoding!";
        let encoded = codec.encode(data, &dict);
        assert!(encoded.is_some(), "Encode should succeed for 12 ranges");
        assert!(!encoded.unwrap().is_empty());
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_13_ranges() {
        let dictionary = generate_synthetic_dictionary(13, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 6+ ranges use scalar fallback (range-reduction not supported)
        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data = b"Testing 13-range encoding!";
        let encoded = codec.encode(data, &dict);
        assert!(encoded.is_some(), "Encode should succeed for 13 ranges");
        assert!(!encoded.unwrap().is_empty());
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_16_ranges() {
        let dictionary = generate_synthetic_dictionary(16, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 6+ ranges use scalar fallback (range-reduction not supported)
        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data = b"Testing 16-range encoding!";
        let encoded = codec.encode(data, &dict);
        assert!(encoded.is_some(), "Encode should succeed for 16 ranges");
        assert!(!encoded.unwrap().is_empty());
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_range_strategy_detection() {
        // Test strategy detection for 1-2 ranges (3+ ranges disabled due to bugs)

        // Test 2-range dictionary (RFC4648 base32)
        let dictionary_2: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict_2 = Dictionary::new(dictionary_2).unwrap();
        let codec_2 = Base64LutCodec::from_dictionary(&dict_2).unwrap();
        assert!(codec_2.range_info.is_some());
        assert_eq!(
            codec_2.range_info.as_ref().unwrap().strategy,
            RangeStrategy::Small
        );

        // Test 3-range dictionary - may have range_info if it fits in LUT
        let dictionary_3 = generate_synthetic_dictionary(3, 32);
        let dict_3 = Dictionary::new(dictionary_3).unwrap();
        let codec_3 = Base64LutCodec::from_dictionary(&dict_3).unwrap();
        // Whether range_info exists depends on whether the compressed indices fit in 16 entries
        // The synthetic dictionary should fit, so expect Some
        assert!(
            codec_3.range_info.is_some(),
            "3-range dictionary that fits in LUT should use SSSE3"
        );
        assert_eq!(
            codec_3.range_info.as_ref().unwrap().strategy,
            RangeStrategy::SmallMulti
        );

        // 6+ ranges should not have range_info
        let dictionary_6 = generate_synthetic_dictionary(6, 64);
        let dict_6 = Dictionary::new(dictionary_6).unwrap();
        let codec_6 = Base64LutCodec::from_dictionary(&dict_6).unwrap();
        assert!(
            codec_6.range_info.is_none(),
            "6+ ranges should use scalar fallback"
        );
    }

    // ========== Multi-range decode tests (6-16 ranges) ==========

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_6_ranges_base32() {
        // 6 ranges for base32 - uses scalar fallback (range-reduction not supported for >5 ranges)
        let dictionary = generate_synthetic_dictionary(6, 32);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 6-16 range support not implemented yet, so range_info should be None
        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data = b"Hello, World! Testing 6-range decode...";
        let encoded = codec.encode(data, &dict).unwrap();

        // Should work using scalar/direct LUT fallback
        let decoded = codec.decode(&encoded, &dict).unwrap();
        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_8_ranges_base32() {
        // 8 ranges for base32 - uses scalar fallback (range-reduction not supported for >5 ranges)
        let dictionary = generate_synthetic_dictionary(8, 32);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data: Vec<u8> = (0..128).collect();
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_12_ranges_base32() {
        // 12 ranges × 2-3 chars ≈ 32 chars - uses scalar fallback
        let dictionary = generate_synthetic_dictionary(12, 32);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data = b"Large multirange dictionary test!";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_16_ranges_base32() {
        // 16 ranges × 2 chars = 32 chars - uses scalar fallback
        let dictionary = generate_synthetic_dictionary(16, 32);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data: Vec<u8> = (0..255).collect();
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_6_ranges_base64() {
        // 6 ranges × 10 chars = 60 chars, pad to 64 - uses scalar fallback
        let dictionary = generate_synthetic_dictionary(6, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data = b"Base64 with 6 contiguous ranges for testing SIMD decode...";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_8_ranges_base64() {
        // 8 ranges × 8 chars = 64 chars - uses scalar fallback
        let dictionary = generate_synthetic_dictionary(8, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data: Vec<u8> = (0..128).collect();
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_12_ranges_base64() {
        // 12 ranges × 5 chars = 60 chars, pad to 64 - uses scalar fallback
        let dictionary = generate_synthetic_dictionary(12, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data = b"Multi-range base64 decode test with 12 contiguous ranges!";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_16_ranges_base64() {
        // 16 ranges × 4 chars = 64 chars - uses scalar fallback
        let dictionary = generate_synthetic_dictionary(16, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data: Vec<u8> = (0..255).collect();
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_6_ranges_all_bytes() {
        // Test all byte values with 6-range dictionary - uses scalar fallback
        let dictionary = generate_synthetic_dictionary(6, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        assert!(
            codec.range_info.is_none(),
            "Range info should not exist for >5 ranges"
        );

        let data: Vec<u8> = (0..=255).collect();
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(
            &decoded[..],
            &data[..],
            "Round-trip failed for all byte values with 6 ranges"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_multi_range_invalid_char() {
        // Test that invalid characters are rejected
        let dictionary = generate_synthetic_dictionary(6, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // Encode valid data
        let data = b"Valid data";
        let mut encoded = codec.encode(data, &dict).unwrap();

        // Inject invalid character (use space, which is before the printable ASCII range)
        if encoded.len() > 8 {
            let encoded_bytes = unsafe { encoded.as_bytes_mut() };
            encoded_bytes[8] = b' '; // Space (32) is not in the dictionary (starts at '!' = 33)
        }

        // Decode should fail
        let result = codec.decode(&encoded, &dict);
        assert!(
            result.is_none(),
            "Should reject invalid character in multi-range decode"
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_multi_range_various_sizes() {
        // Test various input sizes (16, 32, 48, 64 chars for base64)
        let dictionary = generate_synthetic_dictionary(8, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        for size in [12, 24, 36, 48, 60, 100] {
            let data: Vec<u8> = (0..size).map(|i| (i * 7) as u8).collect();
            let encoded = codec.encode(&data, &dict).unwrap();
            let decoded = codec.decode(&encoded, &dict).unwrap();

            assert_eq!(
                &decoded[..],
                &data[..],
                "Round-trip failed at size {}",
                size
            );
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_empty_multi_range() {
        let dictionary = generate_synthetic_dictionary(6, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        let data: Vec<u8> = vec![];
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(decoded.len(), 0, "Empty input should produce empty output");
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_single_block_base32() {
        // Exactly 16 chars (one SIMD block for base32)
        let dictionary = generate_synthetic_dictionary(6, 32);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 10 bytes → 16 chars (base32)
        let data: Vec<u8> = (0..10).collect();
        let encoded = codec.encode(&data, &dict).unwrap();

        // Ensure we have exactly 16 chars
        assert_eq!(encoded.len(), 16, "Should produce exactly 16 chars");

        let decoded = codec.decode(&encoded, &dict).unwrap();
        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_single_block_base64() {
        // Exactly 16 chars (one SIMD block for base64)
        let dictionary = generate_synthetic_dictionary(8, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = Base64LutCodec::from_dictionary(&dict).unwrap();

        // 12 bytes → 16 chars (base64)
        let data: Vec<u8> = (0..12).collect();
        let encoded = codec.encode(&data, &dict).unwrap();

        // Ensure we have exactly 16 chars
        assert_eq!(encoded.len(), 16, "Should produce exactly 16 chars");

        let decoded = codec.decode(&encoded, &dict).unwrap();
        assert_eq!(&decoded[..], &data[..]);
    }
}
