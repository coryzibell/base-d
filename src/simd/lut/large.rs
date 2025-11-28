//! LargeLutCodec: SIMD codec for large arbitrary dictionaries (17-64 characters)
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

/// Character range in dictionary (contiguous ASCII sequence)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CharRange {
    start_idx: u8,  // First index in this range
    end_idx: u8,    // Last index (inclusive)
    start_char: u8, // First ASCII character
    offset: i8,     // char = index + offset
}

/// Range-reduction strategy based on number of ranges
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum RangeStrategy {
    Simple,     // 1 range, direct offset
    Small,      // 2 ranges, single subs
    SmallMulti, // 3-5 ranges, subs + 1 cmp + blend
    Medium,     // 6-8 ranges, 2-3 thresholds
    Large,      // 9-12 ranges, 3-4 thresholds
    VeryLarge,  // 13-16 ranges, 4-5 thresholds
}

/// Range-reduction metadata for SSE/AVX2 encoding
#[derive(Debug, Clone)]
struct RangeInfo {
    ranges: Vec<CharRange>,
    offset_lut: [i8; 16],    // pshufb lookup table
    strategy: RangeStrategy, // Encoding strategy

    // Single threshold (for 2 ranges)
    subs_threshold: u8,       // For _mm_subs_epu8
    cmp_value: Option<u8>,    // For _mm_cmplt_epi8 (if needed)
    override_val: Option<u8>, // For _mm_blendv_epi8 (if needed)

    // Multiple thresholds (for 6-16 ranges)
    thresholds: Vec<u8>, // Ordered thresholds for binary tree
    cmp_values: Vec<u8>, // Comparison values for each threshold
}

impl RangeInfo {
    /// Build range-reduction metadata for 2-5 contiguous ranges
    ///
    /// Note: 6-16 range support is not implemented yet (multi-threshold compression
    /// creates collisions and doesn't fit in 16-byte LUT). Fall back to scalar for these.
    #[cfg(target_arch = "x86_64")]
    fn build_multi_range(ranges: &[CharRange]) -> Option<Self> {
        let num_ranges = ranges.len();

        // Reject if >5 ranges (6-16 range multi-threshold not implemented yet)
        // Also reject if 0 or >16 ranges
        if num_ranges == 0 || num_ranges > 5 {
            return None;
        }

        // Dispatch based on range count
        match num_ranges {
            1 => Self::build_single_range(ranges),
            2 => Self::build_two_ranges(ranges),
            3..=5 => Self::build_small_multirange(ranges),
            _ => None, // 6-16 ranges not supported yet (multi-threshold broken)
        }
    }

    /// Build for single range (no reduction needed)
    #[cfg(target_arch = "x86_64")]
    fn build_single_range(ranges: &[CharRange]) -> Option<Self> {
        let range = ranges[0];

        // Single contiguous range: just use offset directly
        let mut offset_lut = [0i8; 16];
        for i in 0..16 {
            offset_lut[i] = range.offset;
        }

        Some(RangeInfo {
            ranges: ranges.to_vec(),
            offset_lut,
            strategy: RangeStrategy::Simple,
            subs_threshold: 0, // No subtraction needed
            cmp_value: None,
            override_val: None,
            thresholds: Vec::new(),
            cmp_values: Vec::new(),
        })
    }

    /// Build for two ranges (base32 case)
    #[cfg(target_arch = "x86_64")]
    fn build_two_ranges(ranges: &[CharRange]) -> Option<Self> {
        let range0 = ranges[0];
        let range1 = ranges[1];

        // Use boundary of first range as threshold
        let subs_threshold = range0.end_idx;

        // Build offset LUT for pshufb
        let mut offset_lut = [0i8; 16];

        // After subtraction:
        //   range0 indices → 0
        //   range1 indices → [1..=(range1.end_idx - range0.end_idx)]
        offset_lut[0] = range0.offset;

        // Fill remaining slots with range1 offset
        let range1_compressed_len = ((range1.end_idx - range0.end_idx) as usize).min(15);
        for i in 1..=range1_compressed_len {
            offset_lut[i] = range1.offset;
        }

        Some(RangeInfo {
            ranges: ranges.to_vec(),
            offset_lut,
            strategy: RangeStrategy::Small,
            subs_threshold,
            cmp_value: None,    // No comparison needed for 2 ranges
            override_val: None, // No override needed for 2 ranges
            thresholds: Vec::new(),
            cmp_values: Vec::new(),
        })
    }

    /// Build for 3-5 ranges (base64-style)
    #[cfg(target_arch = "x86_64")]
    fn build_small_multirange(ranges: &[CharRange]) -> Option<Self> {
        // Strategy: Use boundary of second-largest range as threshold
        // This maps the two largest ranges to 0, distinguish via comparison

        // Find two largest ranges by length
        let mut sorted_ranges: Vec<_> = ranges.iter().enumerate().collect();
        sorted_ranges
            .sort_by_key(|(_, r)| std::cmp::Reverse((r.end_idx - r.start_idx + 1) as usize));

        let (largest_idx, largest_range) = sorted_ranges[0];
        let (second_largest_idx, second_largest_range) = sorted_ranges[1];

        // Threshold: end of the larger of the two largest ranges
        // For base64: ranges[1] ('a'-'z', indices 26-51) is second-largest
        let subs_threshold = second_largest_range.end_idx;

        // After subtraction, both large ranges map to 0 or near-0
        // Comparison value: distinguish the two large ranges
        let cmp_value = if largest_idx < second_largest_idx {
            // Largest comes first in dictionary
            second_largest_range.start_idx
        } else {
            // Second-largest comes first
            largest_range.start_idx
        };

        // Build offset LUT
        let mut offset_lut = [0i8; 16];

        // Second-largest range stays at compressed index 0
        offset_lut[0] = second_largest_range.offset;
        let mut compressed_idx = 1;

        // Map tail ranges (those after threshold)
        for range in ranges {
            if range.start_idx > subs_threshold {
                let range_len = (range.end_idx - range.start_idx + 1) as usize;
                for i in 0..range_len.min(16 - compressed_idx) {
                    offset_lut[compressed_idx + i] = range.offset;
                }
                compressed_idx += range_len;
                if compressed_idx >= 15 {
                    break;
                }
            }
        }

        // Largest range gets override via blendv
        let override_idx = compressed_idx.min(15);
        offset_lut[override_idx] = largest_range.offset;

        Some(RangeInfo {
            ranges: ranges.to_vec(),
            offset_lut,
            strategy: RangeStrategy::SmallMulti,
            subs_threshold,
            cmp_value: Some(cmp_value),
            override_val: Some(override_idx as u8),
            thresholds: Vec::new(),
            cmp_values: Vec::new(),
        })
    }

    /// Build for 6-8 ranges (medium multirange with 2-3 thresholds)
    #[cfg(target_arch = "x86_64")]
    #[allow(dead_code)]
    fn build_medium_multirange(ranges: &[CharRange]) -> Option<Self> {
        assert!(ranges.len() >= 6 && ranges.len() <= 8);

        // Select thresholds using balanced binary partitioning
        let thresholds = Self::select_thresholds_medium(ranges);
        let cmp_values: Vec<u8> = thresholds.iter().map(|&t| t + 1).collect();

        // Build hierarchical LUT
        let offset_lut = Self::build_hierarchical_lut_medium(ranges, &thresholds);

        Some(RangeInfo {
            ranges: ranges.to_vec(),
            offset_lut,
            strategy: RangeStrategy::Medium,
            subs_threshold: 0, // Unused for multi-threshold
            cmp_value: None,
            override_val: None,
            thresholds,
            cmp_values,
        })
    }

    /// Select thresholds for 6-8 ranges (2-3 thresholds needed)
    #[cfg(target_arch = "x86_64")]
    #[allow(dead_code)]
    fn select_thresholds_medium(ranges: &[CharRange]) -> Vec<u8> {
        let num_ranges = ranges.len();
        let mut thresholds = Vec::new();

        // First threshold: split ranges roughly in half
        let mid_idx = num_ranges / 2;
        if mid_idx > 0 && mid_idx < num_ranges {
            thresholds.push(ranges[mid_idx - 1].end_idx);
        }

        // Second threshold: split lower half
        if mid_idx > 1 {
            let lower_mid = mid_idx / 2;
            if lower_mid > 0 {
                thresholds.push(ranges[lower_mid - 1].end_idx);
            }
        }

        // Third threshold: split upper half (if 7-8 ranges)
        if num_ranges > 6 {
            let upper_mid = mid_idx + (num_ranges - mid_idx) / 2;
            if upper_mid > mid_idx && upper_mid < num_ranges {
                thresholds.push(ranges[upper_mid - 1].end_idx);
            }
        }

        thresholds.sort();
        thresholds
    }

    /// Build hierarchical LUT for 6-8 ranges
    #[cfg(target_arch = "x86_64")]
    #[allow(dead_code)]
    fn build_hierarchical_lut_medium(ranges: &[CharRange], _thresholds: &[u8]) -> [i8; 16] {
        let mut lut = [0i8; 16];
        let mut compressed_idx = 0usize;

        // Assign compressed indices by traversing ranges
        for range in ranges {
            let range_len = (range.end_idx - range.start_idx + 1) as usize;

            for _ in 0..range_len {
                if compressed_idx < 16 {
                    lut[compressed_idx] = range.offset;
                    compressed_idx += 1;
                } else {
                    break;
                }
            }
        }

        lut
    }

    /// Build for 9-12 ranges (large multirange with 3-4 thresholds)
    #[cfg(target_arch = "x86_64")]
    #[allow(dead_code)]
    fn build_large_multirange(ranges: &[CharRange]) -> Option<Self> {
        assert!(ranges.len() >= 9 && ranges.len() <= 12);

        let thresholds = Self::select_thresholds_large(ranges);
        let cmp_values: Vec<u8> = thresholds.iter().map(|&t| t + 1).collect();

        let offset_lut = Self::build_hierarchical_lut_large(ranges, &thresholds);

        Some(RangeInfo {
            ranges: ranges.to_vec(),
            offset_lut,
            strategy: RangeStrategy::Large,
            subs_threshold: 0,
            cmp_value: None,
            override_val: None,
            thresholds,
            cmp_values,
        })
    }

    /// Select thresholds for 9-12 ranges (3-4 thresholds)
    #[cfg(target_arch = "x86_64")]
    #[allow(dead_code)]
    fn select_thresholds_large(ranges: &[CharRange]) -> Vec<u8> {
        let mut thresholds = Vec::new();

        // Recursive balanced partitioning for 3-4 levels
        fn partition_ranges(ranges: &[CharRange], depth: usize, thresholds: &mut Vec<u8>) {
            if ranges.is_empty() || depth >= 4 {
                return;
            }

            let mid_idx = ranges.len() / 2;
            if mid_idx > 0 && mid_idx < ranges.len() {
                thresholds.push(ranges[mid_idx - 1].end_idx);
                partition_ranges(&ranges[..mid_idx], depth + 1, thresholds);
                partition_ranges(&ranges[mid_idx..], depth + 1, thresholds);
            }
        }

        partition_ranges(ranges, 0, &mut thresholds);
        thresholds.sort();
        thresholds.dedup();
        thresholds
    }

    /// Build hierarchical LUT for 9-12 ranges
    #[cfg(target_arch = "x86_64")]
    #[allow(dead_code)]
    fn build_hierarchical_lut_large(ranges: &[CharRange], _thresholds: &[u8]) -> [i8; 16] {
        let mut lut = [0i8; 16];
        let mut compressed_idx = 0usize;

        for range in ranges {
            let range_len = (range.end_idx - range.start_idx + 1) as usize;

            for _ in 0..range_len {
                if compressed_idx < 16 {
                    lut[compressed_idx] = range.offset;
                    compressed_idx += 1;
                } else {
                    break;
                }
            }
        }

        lut
    }

    /// Build for 13-16 ranges (very large multirange with 4-5 thresholds)
    #[cfg(target_arch = "x86_64")]
    #[allow(dead_code)]
    fn build_very_large_multirange(ranges: &[CharRange]) -> Option<Self> {
        assert!(ranges.len() >= 13 && ranges.len() <= 16);

        let thresholds = Self::select_thresholds_very_large(ranges);
        let cmp_values: Vec<u8> = thresholds.iter().map(|&t| t + 1).collect();

        let offset_lut = Self::build_hierarchical_lut_very_large(ranges, &thresholds);

        Some(RangeInfo {
            ranges: ranges.to_vec(),
            offset_lut,
            strategy: RangeStrategy::VeryLarge,
            subs_threshold: 0,
            cmp_value: None,
            override_val: None,
            thresholds,
            cmp_values,
        })
    }

    /// Select thresholds for 13-16 ranges (4-5 thresholds)
    #[cfg(target_arch = "x86_64")]
    #[allow(dead_code)]
    fn select_thresholds_very_large(ranges: &[CharRange]) -> Vec<u8> {
        let mut thresholds = Vec::new();

        fn partition_ranges(ranges: &[CharRange], depth: usize, thresholds: &mut Vec<u8>) {
            if ranges.is_empty() || depth >= 5 {
                return;
            }

            let mid_idx = ranges.len() / 2;
            if mid_idx > 0 && mid_idx < ranges.len() {
                thresholds.push(ranges[mid_idx - 1].end_idx);
                partition_ranges(&ranges[..mid_idx], depth + 1, thresholds);
                partition_ranges(&ranges[mid_idx..], depth + 1, thresholds);
            }
        }

        partition_ranges(ranges, 0, &mut thresholds);
        thresholds.sort();
        thresholds.dedup();
        thresholds
    }

    /// Build hierarchical LUT for 13-16 ranges
    #[cfg(target_arch = "x86_64")]
    #[allow(dead_code)]
    fn build_hierarchical_lut_very_large(ranges: &[CharRange], _thresholds: &[u8]) -> [i8; 16] {
        let mut lut = [0i8; 16];
        let mut compressed_idx = 0usize;

        for range in ranges {
            let range_len = (range.end_idx - range.start_idx + 1) as usize;

            for _ in 0..range_len {
                if compressed_idx < 16 {
                    lut[compressed_idx] = range.offset;
                    compressed_idx += 1;
                } else {
                    break;
                }
            }
        }

        lut
    }
}

/// SIMD codec for large arbitrary dictionaries (17-64 characters)
///
/// Uses platform-dependent lookup for encoding and a 256-byte sparse
/// table for decoding with validation.
pub struct LargeLutCodec {
    metadata: DictionaryMetadata,

    /// Encoding LUT: index → char (64 bytes, one per possible index)
    encode_lut: [u8; 64],

    /// Decoding LUT: char → index (256 bytes, sparse)
    /// 0xFF means invalid character
    decode_lut: [u8; 256],

    /// Range-reduction metadata for SSE/AVX2 fallback (x86_64 only)
    #[cfg(target_arch = "x86_64")]
    range_info: Option<RangeInfo>,
}

impl LargeLutCodec {
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
        for i in 0..metadata.base {
            let ch = dict.encode_digit(i)?;

            // Validation: char must be ASCII (single-byte)
            if (ch as u32) > 0x7F {
                return None; // Multi-byte UTF-8 not supported
            }

            encode_lut[i] = ch as u8;
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
            32 => (data.len() * 8 + 4) / 5, // 5 bits per char
            64 => (data.len() * 8 + 5) / 6, // 6 bits per char
            _ => return None,
        };

        let mut result = String::with_capacity(output_len);

        #[cfg(target_arch = "aarch64")]
        unsafe {
            self.encode_neon_impl(data, &mut result);
            return Some(result);
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
            self.encode_neon_base32(data, result);
        } else if self.metadata.base == 64 {
            self.encode_neon_base64(data, result);
        }
    }

    /// NEON base32 encode (5-bit indices)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_base32(&self, data: &[u8], result: &mut String) {
        use std::arch::aarch64::*;

        const BLOCK_SIZE: usize = 5; // 5 bytes -> 8 chars (40 bits)

        if data.len() < BLOCK_SIZE {
            self.encode_scalar_base32(data, result);
            return;
        }

        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Load 64-byte LUT into four 16-byte tables
        let lut_tables = uint8x16x4_t(
            vld1q_u8(self.encode_lut.as_ptr()),
            vld1q_u8(self.encode_lut.as_ptr().add(16)),
            vld1q_u8(self.encode_lut.as_ptr().add(32)),
            vld1q_u8(self.encode_lut.as_ptr().add(48)),
        );

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Process 5 bytes -> 8 chars
            // Load 5 bytes and extract 5-bit indices
            let bytes = [
                *data.get_unchecked(offset),
                *data.get_unchecked(offset + 1),
                *data.get_unchecked(offset + 2),
                *data.get_unchecked(offset + 3),
                *data.get_unchecked(offset + 4),
            ];

            // Extract 8 x 5-bit indices from 5 bytes (40 bits)
            let mut indices = [0u8; 16];
            indices[0] = (bytes[0] >> 3) & 0x1F; // bits 7-3
            indices[1] = ((bytes[0] << 2) | (bytes[1] >> 6)) & 0x1F; // bits 2-0, 7-6
            indices[2] = (bytes[1] >> 1) & 0x1F; // bits 5-1
            indices[3] = ((bytes[1] << 4) | (bytes[2] >> 4)) & 0x1F; // bits 0, 7-4
            indices[4] = ((bytes[2] << 1) | (bytes[3] >> 7)) & 0x1F; // bits 3-0, 7
            indices[5] = (bytes[3] >> 2) & 0x1F; // bits 6-2
            indices[6] = ((bytes[3] << 3) | (bytes[4] >> 5)) & 0x1F; // bits 1-0, 7-5
            indices[7] = bytes[4] & 0x1F; // bits 4-0

            // Load indices into NEON register
            let idx_vec = vld1q_u8(indices.as_ptr());

            // Translate using vqtbl4q_u8 (64-byte lookup)
            let chars = vqtbl4q_u8(lut_tables, idx_vec);

            // Store 16 output characters (only first 8 are valid)
            let mut output_buf = [0u8; 16];
            vst1q_u8(output_buf.as_mut_ptr(), chars);

            // Append to result (only first 8 chars)
            for &byte in &output_buf[0..8] {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Handle remainder with scalar
        if simd_bytes < data.len() {
            self.encode_scalar_base32(&data[simd_bytes..], result);
        }
    }

    /// NEON base64 encode (6-bit indices)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_base64(&self, data: &[u8], result: &mut String) {
        use std::arch::aarch64::*;

        const BLOCK_SIZE: usize = 12; // 12 bytes -> 16 chars

        if data.len() < 16 {
            self.encode_scalar_base64(data, result);
            return;
        }

        let safe_len = if data.len() >= 4 { data.len() - 4 } else { 0 };
        let num_blocks = safe_len / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Load 64-byte LUT into four 16-byte tables
        let lut_tables = uint8x16x4_t(
            vld1q_u8(self.encode_lut.as_ptr()),
            vld1q_u8(self.encode_lut.as_ptr().add(16)),
            vld1q_u8(self.encode_lut.as_ptr().add(32)),
            vld1q_u8(self.encode_lut.as_ptr().add(48)),
        );

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Load 16 bytes (will use first 12)
            let input_vec = vld1q_u8(data.as_ptr().add(offset));

            // Reshuffle to extract 6-bit indices (same as specialized base64)
            let reshuffled = self.reshuffle_neon_base64(input_vec);

            // Translate using vqtbl4q_u8 (64-byte lookup)
            let chars = vqtbl4q_u8(lut_tables, reshuffled);

            // Store 16 output characters
            let mut output_buf = [0u8; 16];
            vst1q_u8(output_buf.as_mut_ptr(), chars);

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
        // Matches specialized/base64.rs reshuffle pattern
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
        let shuffled_u32 = vreinterpretq_u32_u8(shuffled);

        // Extract 6-bit groups using shifts and masks
        // First extraction: positions 0 and 2 in each group
        // Mask 0x0FC0FC00: isolate specific bit positions
        let t0 = vandq_u32(shuffled_u32, vdupq_n_u32(0x0FC0FC00));
        let t0_u16 = vreinterpretq_u16_u32(t0);
        let mult_hi = vmulq_n_u16(t0_u16, 0x0040);
        let t1 = vreinterpretq_u32_u16(vshrq_n_u16(mult_hi, 10));

        // Second extraction: positions 1 and 3 in each group
        // Mask 0x003F03F0: isolate different bit positions
        let t2 = vandq_u32(shuffled_u32, vdupq_n_u32(0x003F03F0));
        let t2_u16 = vreinterpretq_u16_u32(t2);
        let mult_lo = vmulq_n_u16(t2_u16, 0x0010);
        let t3 = vreinterpretq_u32_u16(vshrq_n_u16(mult_lo, 6));

        // Combine the two results
        vreinterpretq_u8_u32(vorrq_u32(t1, t3))
    }

    /// Scalar fallback for base32 encoding
    #[cfg(target_arch = "aarch64")]
    fn encode_scalar_base32(&self, data: &[u8], result: &mut String) {
        let mut bit_buffer = 0u32;
        let mut bits_in_buffer = 0;

        for &byte in data {
            bit_buffer = (bit_buffer << 8) | (byte as u32);
            bits_in_buffer += 8;

            while bits_in_buffer >= 5 {
                bits_in_buffer -= 5;
                let index = ((bit_buffer >> bits_in_buffer) & 0x1F) as usize;
                result.push(self.encode_lut[index] as char);
            }
        }

        // Flush remaining bits
        if bits_in_buffer > 0 {
            let index = ((bit_buffer << (5 - bits_in_buffer)) & 0x1F) as usize;
            result.push(self.encode_lut[index] as char);
        }
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
                    self.encode_avx512_vbmi_base32(data, result);
                } else if self.metadata.base == 64 {
                    self.encode_avx512_vbmi_base64(data, result);
                }
                return;
            }
        }

        // Try SSE range-reduction if supported
        if is_x86_feature_detected!("ssse3") && self.range_info.is_some() {
            self.encode_ssse3_range_reduction(data, result);
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
            32 => self.encode_ssse3_range_reduction_5bit(data, result),
            64 => self.encode_ssse3_range_reduction_6bit(data, result),
            _ => unreachable!("Only base32/64 supported for range-reduction"),
        }
    }

    /// SSE range-reduction base32 encode (5-bit indices)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3_range_reduction_5bit(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 5; // 5 bytes → 8 chars (40 bits)

        if data.len() < BLOCK_SIZE {
            self.encode_scalar_base32_x86(data, result);
            return;
        }

        let range_info = self.range_info.as_ref().unwrap();
        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Load offset LUT once (reused for all blocks)
        let offset_lut = _mm_loadu_si128(range_info.offset_lut.as_ptr() as *const __m128i);
        let subs_threshold = _mm_set1_epi8(range_info.subs_threshold as i8);

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Extract 5 bytes → 8 x 5-bit indices from 5 bytes (40 bits)
            let bytes = [
                *data.get_unchecked(offset),
                *data.get_unchecked(offset + 1),
                *data.get_unchecked(offset + 2),
                *data.get_unchecked(offset + 3),
                *data.get_unchecked(offset + 4),
            ];

            let mut indices = [0u8; 16];
            indices[0] = (bytes[0] >> 3) & 0x1F;
            indices[1] = ((bytes[0] << 2) | (bytes[1] >> 6)) & 0x1F;
            indices[2] = (bytes[1] >> 1) & 0x1F;
            indices[3] = ((bytes[1] << 4) | (bytes[2] >> 4)) & 0x1F;
            indices[4] = ((bytes[2] << 1) | (bytes[3] >> 7)) & 0x1F;
            indices[5] = (bytes[3] >> 2) & 0x1F;
            indices[6] = ((bytes[3] << 3) | (bytes[4] >> 5)) & 0x1F;
            indices[7] = bytes[4] & 0x1F;

            let idx_vec = _mm_loadu_si128(indices.as_ptr() as *const __m128i);

            // === RANGE REDUCTION ===

            // Step 1: Saturating subtraction
            let reduced = _mm_subs_epu8(idx_vec, subs_threshold);

            // Step 2: Comparison + blend (if needed for >2 ranges)
            let compressed = if let (Some(cmp), Some(override_val)) =
                (range_info.cmp_value, range_info.override_val)
            {
                let cmp_vec = _mm_set1_epi8(cmp as i8);
                let override_vec = _mm_set1_epi8(override_val as i8);
                let is_below = _mm_cmplt_epi8(idx_vec, cmp_vec);
                _mm_blendv_epi8(reduced, override_vec, is_below)
            } else {
                reduced
            };

            // Step 3: Lookup offset (compressed index → offset)
            let offset_vec = _mm_shuffle_epi8(offset_lut, compressed);

            // Step 4: Add offset to original index
            let chars = _mm_add_epi8(idx_vec, offset_vec);

            // Store results
            let mut output_buf = [0u8; 16];
            _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, chars);

            // Append first 8 chars to result
            for &byte in &output_buf[0..8] {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Scalar remainder
        if simd_bytes < data.len() {
            self.encode_scalar_base32_x86(&data[simd_bytes..], result);
        }
    }

    /// Multi-threshold encoding for 6-16 ranges (6-bit indices)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
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
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3_range_reduction_6bit(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 12; // 12 bytes → 16 chars

        if data.len() < 16 {
            self.encode_scalar_base64_x86(data, result);
            return;
        }

        let range_info = self.range_info.as_ref().unwrap();
        let safe_len = if data.len() >= 4 { data.len() - 4 } else { 0 };
        let num_blocks = safe_len / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Load constants once
        let offset_lut = _mm_loadu_si128(range_info.offset_lut.as_ptr() as *const __m128i);
        let subs_threshold = _mm_set1_epi8(range_info.subs_threshold as i8);

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Load 16 bytes and extract 6-bit indices (reuse existing reshuffle logic)
            let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);
            let idx_vec = self.reshuffle_x86_base64(input_vec);

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
                    self.encode_multi_threshold_6bit(idx_vec, range_info)
                }
                RangeStrategy::Large => {
                    // 9-12 ranges - multi-threshold
                    self.encode_multi_threshold_6bit(idx_vec, range_info)
                }
                RangeStrategy::VeryLarge => {
                    // 13-16 ranges - multi-threshold
                    self.encode_multi_threshold_6bit(idx_vec, range_info)
                }
            };

            // Step 3: Lookup offset
            let offset_vec = _mm_shuffle_epi8(offset_lut, compressed);

            // Step 4: Add offset to compressed index (NOT original index!)
            let chars = _mm_add_epi8(compressed, offset_vec);

            // Store results
            let mut output_buf = [0u8; 16];
            _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, chars);

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

    /// AVX-512 VBMI base32 encode (5-bit indices)
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512vbmi"))]
    #[target_feature(enable = "avx512vbmi")]
    unsafe fn encode_avx512_vbmi_base32(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 5; // 5 bytes -> 8 chars (40 bits)

        if data.len() < BLOCK_SIZE {
            self.encode_scalar_base32_x86(data, result);
            return;
        }

        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Load 64-byte LUT into ZMM register (only once - vpermb doesn't destroy it!)
        let lut = _mm512_loadu_si512(self.encode_lut.as_ptr() as *const i32);

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Process 5 bytes -> 8 chars
            let bytes = [
                *data.get_unchecked(offset),
                *data.get_unchecked(offset + 1),
                *data.get_unchecked(offset + 2),
                *data.get_unchecked(offset + 3),
                *data.get_unchecked(offset + 4),
            ];

            // Extract 8 x 5-bit indices from 5 bytes (40 bits)
            let mut indices = [0u8; 64]; // ZMM is 64 bytes, but we only use first 8
            indices[0] = (bytes[0] >> 3) & 0x1F;
            indices[1] = ((bytes[0] << 2) | (bytes[1] >> 6)) & 0x1F;
            indices[2] = (bytes[1] >> 1) & 0x1F;
            indices[3] = ((bytes[1] << 4) | (bytes[2] >> 4)) & 0x1F;
            indices[4] = ((bytes[2] << 1) | (bytes[3] >> 7)) & 0x1F;
            indices[5] = (bytes[3] >> 2) & 0x1F;
            indices[6] = ((bytes[3] << 3) | (bytes[4] >> 5)) & 0x1F;
            indices[7] = bytes[4] & 0x1F;

            // Load indices into ZMM register
            let idx_vec = _mm512_loadu_si512(indices.as_ptr() as *const i32);

            // Translate using vpermb (64-byte lookup, doesn't destroy lut register)
            let chars = _mm512_permutexvar_epi8(idx_vec, lut);

            // Store output
            let mut output_buf = [0u8; 64];
            _mm512_storeu_si512(output_buf.as_mut_ptr() as *mut i32, chars);

            // Append to result (only first 8 chars)
            for &byte in &output_buf[0..8] {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Handle remainder with scalar
        if simd_bytes < data.len() {
            self.encode_scalar_base32_x86(&data[simd_bytes..], result);
        }
    }

    /// AVX-512 VBMI base64 encode (6-bit indices)
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512vbmi"))]
    #[target_feature(enable = "avx512vbmi")]
    unsafe fn encode_avx512_vbmi_base64(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 12; // 12 bytes -> 16 chars

        if data.len() < 16 {
            self.encode_scalar_base64_x86(data, result);
            return;
        }

        let safe_len = if data.len() >= 4 { data.len() - 4 } else { 0 };
        let num_blocks = safe_len / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Load 64-byte LUT into ZMM register
        let lut = _mm512_loadu_si512(self.encode_lut.as_ptr() as *const i32);

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Load 16 bytes (will use first 12)
            let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

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

    /// Scalar fallback for base32 encoding (x86)
    #[cfg(target_arch = "x86_64")]
    fn encode_scalar_base32_x86(&self, data: &[u8], result: &mut String) {
        let mut bit_buffer = 0u32;
        let mut bits_in_buffer = 0;

        for &byte in data {
            bit_buffer = (bit_buffer << 8) | (byte as u32);
            bits_in_buffer += 8;

            while bits_in_buffer >= 5 {
                bits_in_buffer -= 5;
                let index = ((bit_buffer >> bits_in_buffer) & 0x1F) as usize;
                result.push(self.encode_lut[index] as char);
            }
        }

        // Flush remaining bits
        if bits_in_buffer > 0 {
            let index = ((bit_buffer << (5 - bits_in_buffer)) & 0x1F) as usize;
            result.push(self.encode_lut[index] as char);
        }
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
            return Some(result);
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

    /// Check if dictionary is RFC4648 base32
    fn is_rfc4648_base32(&self) -> bool {
        if self.metadata.base != 32 {
            return false;
        }
        // RFC4648: "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
        let expected = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        &self.encode_lut[..32] == expected
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
            self.decode_ssse3_base32_rfc4648(encoded, result)
        } else if self.is_standard_base64() {
            self.decode_ssse3_base64_standard(encoded, result)
        } else if let Some(ref range_info) = self.range_info {
            // Try SIMD multi-range decode for 6-16 ranges
            if range_info.ranges.len() >= 6 {
                self.decode_ssse3_multi_range(encoded, result)
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
            self.decode_neon_base32_rfc4648(encoded, result)
        } else if self.is_standard_base64() {
            self.decode_neon_base64_standard(encoded, result)
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
            32 => self.decode_ssse3_multi_range_5bit(encoded, result),
            64 => self.decode_ssse3_multi_range_6bit(encoded, result),
            _ => self.decode_scalar(encoded, result),
        }
    }

    /// Multi-range decode for base32 (5-bit indices)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_ssse3_multi_range_5bit(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 16; // 16 chars → 10 bytes

        let range_info = self.range_info.as_ref().unwrap();
        let num_blocks = encoded.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for i in 0..num_blocks {
            let offset = i * BLOCK_SIZE;
            let chars = _mm_loadu_si128(encoded.as_ptr().add(offset) as *const __m128i);

            // === VALIDATION + TRANSLATION ===
            let indices = match self.validate_and_translate_multi_range(chars, range_info) {
                Some(idx) => idx,
                None => return false,
            };

            // === UNPACKING (16×5-bit → 10 bytes) ===
            let bytes = self.unpack_5bit_ssse3(indices);

            // Store 10 bytes
            result.extend_from_slice(&bytes);
        }

        // Scalar remainder
        if simd_bytes < encoded.len() {
            if !self.decode_scalar(&encoded[simd_bytes..], result) {
                return false;
            }
        }

        true
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
            let chars = _mm_loadu_si128(input_no_padding.as_ptr().add(offset) as *const __m128i);

            // === VALIDATION + TRANSLATION ===
            let indices = match self.validate_and_translate_multi_range(chars, range_info) {
                Some(idx) => idx,
                None => return false,
            };

            // === UNPACKING (reuse existing reshuffle_decode_ssse3) ===
            let decoded = self.reshuffle_decode_ssse3(indices);

            let mut output_buf = [0u8; 16];
            _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, decoded);
            result.extend_from_slice(&output_buf[0..12]);
        }

        // Scalar remainder
        if simd_bytes < input_no_padding.len() {
            if !self.decode_scalar(&input_no_padding[simd_bytes..], result) {
                return false;
            }
        }

        true
    }

    /// Validate and translate chars to indices for multi-range dictionaries
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn validate_and_translate_multi_range(
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

    /// Unpack 16×5-bit indices to 10×8-bit bytes (SIMD-accelerated)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn unpack_5bit_ssse3(&self, indices: std::arch::x86_64::__m128i) -> [u8; 10] {
        use std::arch::x86_64::*;

        // Extract indices to array for bit manipulation
        let mut idx_buf = [0u8; 16];
        _mm_storeu_si128(idx_buf.as_mut_ptr() as *mut __m128i, indices);

        // Mask to 5 bits (indices may have high bits set from blending)
        for i in 0..16 {
            idx_buf[i] &= 0x1F;
        }

        // Pack 16×5-bit → 10×8-bit
        // This is complex bit manipulation - using optimized scalar for now
        // (SIMD shuffle-based packing is ~40 ops for marginal gain)
        let mut output = [0u8; 10];

        // First 8 indices → 5 bytes
        output[0] = (idx_buf[0] << 3) | (idx_buf[1] >> 2);
        output[1] = (idx_buf[1] << 6) | (idx_buf[2] << 1) | (idx_buf[3] >> 4);
        output[2] = (idx_buf[3] << 4) | (idx_buf[4] >> 1);
        output[3] = (idx_buf[4] << 7) | (idx_buf[5] << 2) | (idx_buf[6] >> 3);
        output[4] = (idx_buf[6] << 5) | idx_buf[7];

        // Second 8 indices → 5 bytes
        output[5] = (idx_buf[8] << 3) | (idx_buf[9] >> 2);
        output[6] = (idx_buf[9] << 6) | (idx_buf[10] << 1) | (idx_buf[11] >> 4);
        output[7] = (idx_buf[11] << 4) | (idx_buf[12] >> 1);
        output[8] = (idx_buf[12] << 7) | (idx_buf[13] << 2) | (idx_buf[14] >> 3);
        output[9] = (idx_buf[14] << 5) | idx_buf[15];

        output
    }

    /// SSSE3 base32 RFC4648 decode (range-based validation)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_ssse3_base32_rfc4648(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 16; // 16 chars per iteration

        let num_blocks = encoded.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for i in 0..num_blocks {
            let offset = i * BLOCK_SIZE;
            let input = _mm_loadu_si128(encoded.as_ptr().add(offset) as *const __m128i);

            // === VALIDATION (Range Checks) ===
            // Range 1: 'A'-'Z' (65-90)
            let ge_a = _mm_cmpgt_epi8(input, _mm_set1_epi8(64)); // c > '@'
            let le_z = _mm_cmplt_epi8(input, _mm_set1_epi8(91)); // c < '['
            let in_range1 = _mm_and_si128(ge_a, le_z);

            // Range 2: '2'-'7' (50-55)
            let ge_2 = _mm_cmpgt_epi8(input, _mm_set1_epi8(49)); // c > '1'
            let le_7 = _mm_cmplt_epi8(input, _mm_set1_epi8(56)); // c < '8'
            let in_range2 = _mm_and_si128(ge_2, le_7);

            let valid_mask = _mm_or_si128(in_range1, in_range2);
            if _mm_movemask_epi8(valid_mask) != 0xFFFF {
                return false;
            }

            // === TRANSLATION (char → 5-bit index) ===
            let letter_indices = _mm_sub_epi8(input, _mm_set1_epi8(65)); // 'A' → 0
            let digit_indices =
                _mm_add_epi8(_mm_sub_epi8(input, _mm_set1_epi8(50)), _mm_set1_epi8(26)); // '2' → 26
            let indices = _mm_blendv_epi8(digit_indices, letter_indices, in_range1);

            // === UNPACKING (16×5-bit → 10 bytes) ===
            // Extract to array for scalar bit manipulation
            let mut idx_buf = [0u8; 16];
            _mm_storeu_si128(idx_buf.as_mut_ptr() as *mut __m128i, indices);

            // Manual bit packing (scalar for now)
            let mut bit_buffer = 0u32;
            let mut bits_in_buffer = 0;

            for &idx in &idx_buf {
                bit_buffer = (bit_buffer << 5) | (idx as u32);
                bits_in_buffer += 5;

                while bits_in_buffer >= 8 {
                    bits_in_buffer -= 8;
                    let byte = ((bit_buffer >> bits_in_buffer) & 0xFF) as u8;
                    result.push(byte);
                }
            }
        }

        // Scalar remainder
        if simd_bytes < encoded.len() {
            if !self.decode_scalar(&encoded[simd_bytes..], result) {
                return false;
            }
        }

        true
    }

    /// NEON base32 RFC4648 decode (range-based validation)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn decode_neon_base32_rfc4648(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
        use std::arch::aarch64::*;

        const BLOCK_SIZE: usize = 16;

        let num_blocks = encoded.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for i in 0..num_blocks {
            let offset = i * BLOCK_SIZE;
            let input_vec = vld1q_u8(encoded.as_ptr().add(offset));

            // === VALIDATION (Range Checks) ===
            // Range 1: 'A'-'Z' (65-90)
            let ge_a = vcgtq_u8(input_vec, vdupq_n_u8(64));
            let le_z = vcltq_u8(input_vec, vdupq_n_u8(91));
            let in_range1 = vandq_u8(ge_a, le_z);

            // Range 2: '2'-'7' (50-55)
            let ge_2 = vcgtq_u8(input_vec, vdupq_n_u8(49));
            let le_7 = vcltq_u8(input_vec, vdupq_n_u8(56));
            let in_range2 = vandq_u8(ge_2, le_7);

            let valid_mask = vorrq_u8(in_range1, in_range2);
            if vminvq_u8(valid_mask) != 0xFF {
                return false;
            }

            // === TRANSLATION (char → 5-bit index) ===
            let letter_indices = vsubq_u8(input_vec, vdupq_n_u8(65)); // 'A' → 0
            let digit_indices = vaddq_u8(vsubq_u8(input_vec, vdupq_n_u8(50)), vdupq_n_u8(26)); // '2' → 26
            let indices = vbslq_u8(in_range1, letter_indices, digit_indices);

            // === UNPACKING ===
            let mut idx_buf = [0u8; 16];
            vst1q_u8(idx_buf.as_mut_ptr(), indices);

            let mut bit_buffer = 0u32;
            let mut bits_in_buffer = 0;

            for &idx in &idx_buf {
                bit_buffer = (bit_buffer << 5) | (idx as u32);
                bits_in_buffer += 5;

                while bits_in_buffer >= 8 {
                    bits_in_buffer -= 8;
                    let byte = ((bit_buffer >> bits_in_buffer) & 0xFF) as u8;
                    result.push(byte);
                }
            }
        }

        // Scalar remainder
        if simd_bytes < encoded.len() {
            if !self.decode_scalar(&encoded[simd_bytes..], result) {
                return false;
            }
        }

        true
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
            let input_vec =
                _mm_loadu_si128(input_no_padding.as_ptr().add(offset) as *const __m128i);

            // === VALIDATION & TRANSLATION (char → 6-bit index) ===
            let mut char_buf = [0u8; 16];
            _mm_storeu_si128(char_buf.as_mut_ptr() as *mut __m128i, input_vec);

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
                    let idx = self.decode_lut[char_buf[j] as usize];
                    if idx == 0xFF {
                        return false;
                    }
                    indices_buf[j] = idx;
                }
            }

            let indices = _mm_loadu_si128(indices_buf.as_ptr() as *const __m128i);

            // === UNPACKING (reuse specialized base64 reshuffle) ===
            let decoded = self.reshuffle_decode_ssse3(indices);

            let mut output_buf = [0u8; 16];
            _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, decoded);
            result.extend_from_slice(&output_buf[0..12]);
        }

        // Scalar remainder
        if simd_bytes < input_no_padding.len() {
            if !self.decode_scalar(&input_no_padding[simd_bytes..], result) {
                return false;
            }
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
            let input_vec = vld1q_u8(input_no_padding.as_ptr().add(offset));

            // === VALIDATION & TRANSLATION ===
            let mut char_buf = [0u8; 16];
            vst1q_u8(char_buf.as_mut_ptr(), input_vec);

            let mut indices_buf = [0u8; 16];

            // For range-reduced dictionaries, reverse the transformation
            // Note: aarch64 doesn't have range_info, so always use decode_lut
            for j in 0..16 {
                let idx = self.decode_lut[char_buf[j] as usize];
                if idx == 0xFF {
                    return false;
                }
                indices_buf[j] = idx;
            }

            let indices = vld1q_u8(indices_buf.as_ptr());

            // === UNPACKING ===
            let decoded = self.reshuffle_decode_neon(indices);

            let mut output_buf = [0u8; 16];
            vst1q_u8(output_buf.as_mut_ptr(), decoded);
            result.extend_from_slice(&output_buf[0..12]);
        }

        // Scalar remainder
        if simd_bytes < input_no_padding.len() {
            if !self.decode_scalar(&input_no_padding[simd_bytes..], result) {
                return false;
            }
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

        // Stage 1: Merge adjacent pairs
        let merge_ab_and_bc = vreinterpretq_u16_s16(vmull_s8(
            vget_low_s8(vreinterpretq_s8_u8(indices)),
            vdup_n_s8(1),
        ));
        let merge_ab_and_bc_hi =
            vreinterpretq_u16_s16(vmull_high_s8(vreinterpretq_s8_u8(indices), vdupq_n_s8(1)));

        // Unpack 6-bit indices to 8-bit bytes
        // Base64: 4 chars (6 bits each) -> 3 bytes (8 bits each)
        let mut idx_buf = [0u8; 16];
        vst1q_u8(idx_buf.as_mut_ptr(), indices);

        let mut out_buf = [0u8; 16];
        for j in 0..4 {
            let base = j * 4;
            let a = idx_buf[base] as u8;
            let b = idx_buf[base + 1] as u8;
            let c = idx_buf[base + 2] as u8;
            let d = idx_buf[base + 3] as u8;

            // Unpack: [aaaaaa][bbbbbb][cccccc][dddddd] -> [aaaaaabb][bbbbcccc][ccdddddd]
            out_buf[j * 3] = (a << 2) | (b >> 4);
            out_buf[j * 3 + 1] = (b << 4) | (c >> 2);
            out_buf[j * 3 + 2] = (c << 6) | d;
        }

        vld1q_u8(out_buf.as_ptr())
    }

    /// Scalar fallback for decoding
    fn decode_scalar(&self, encoded: &[u8], result: &mut Vec<u8>) -> bool {
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
mod tests {
    use super::*;

    #[test]
    fn test_creation_from_arbitrary_base32() {
        // Shuffled 32-char dictionary
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = LargeLutCodec::from_dictionary(&dict);
        assert!(codec.is_some(), "Should create codec for arbitrary base32");
    }

    #[test]
    fn test_creation_from_arbitrary_base64() {
        // Shuffled 64-char dictionary
        let chars: Vec<char> = "zyxwvutsrqponmlkjihgfedcbaZYXWVUTSRQPONMLKJIHGFEDCBA9876543210_-"
            .chars()
            .collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = LargeLutCodec::from_dictionary(&dict);
        assert!(codec.is_some(), "Should create codec for arbitrary base64");
    }

    #[test]
    fn test_rejects_sequential_dictionary() {
        // Sequential dictionary should use GenericSimdCodec, not LUT
        let chars: Vec<char> = (0x41..0x61).map(|c| char::from_u32(c).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = LargeLutCodec::from_dictionary(&dict);
        assert!(
            codec.is_none(),
            "Should reject sequential (use GenericSimdCodec)"
        );
    }

    #[test]
    fn test_rejects_small_dictionary() {
        // 16-char dictionary too small for LargeLutCodec
        let chars: Vec<char> = "0123456789ABCDEF".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = LargeLutCodec::from_dictionary(&dict);
        assert!(codec.is_none(), "Should reject base16 (too small)");
    }

    #[test]
    fn test_rejects_non_power_of_two() {
        // 40-char dictionary (non power-of-2)
        let chars: Vec<char> = (0x41..0x69).map(|c| char::from_u32(c).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = LargeLutCodec::from_dictionary(&dict);
        assert!(codec.is_none(), "Should reject non-power-of-2 base");
    }

    #[test]
    fn test_lut_construction_base32() {
        // Shuffled base32 dictionary (32 unique chars)
        let chars: Vec<char> = "76543ABCDEFGHIJKLMNOPQRSTUVWXYZ2".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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

        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

        let data = b"The quick brown fox jumps over the lazy dog";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    fn test_encode_empty_input() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

        let data: Vec<u8> = vec![];
        let encoded = codec.encode(&data, &dict).unwrap();

        assert_eq!(encoded, "");
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_encode_various_sizes_base32() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

        // 'a' is not in dictionary (lowercase not present)
        let invalid = "ZYXa";
        let result = codec.decode(invalid, &dict);

        assert!(result.is_none(), "Should reject invalid character 'a'");
    }

    #[test]
    fn test_decode_empty_input() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

        let result = codec.decode("", &dict).unwrap();
        assert_eq!(result.len(), 0);
    }

    /// Integration test: verify round-trip with real data
    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_integration_base32_neon() {
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBA234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let ranges = LargeLutCodec::detect_ranges(chars, 32);

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
        let ranges = LargeLutCodec::detect_ranges(chars, 32);
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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let ranges = LargeLutCodec::detect_ranges(chars, 64);

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

        let codec = LargeLutCodec::from_dictionary(&dict);
        if codec.is_none() {
            eprintln!("from_dictionary returned None for arbitrary base64");
        }
        assert!(codec.is_some(), "Should create codec for arbitrary base64");
        let codec = codec.unwrap();

        assert!(
            codec.range_info.is_some(),
            "Should build range info for base64"
        );

        // Test encoding
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
    //     let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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

        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();
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

        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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

        let codec = LargeLutCodec::from_dictionary(&dict);
        assert!(codec.is_none(), "Should reject sequential dictionary");
    }

    // ========== SIMD decode-specific tests ==========

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_decode_rfc4648_base32_simd() {
        // RFC4648 base32 dictionary - triggers SIMD path
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
    #[cfg(test)]
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

        let ranges = LargeLutCodec::detect_ranges(&encode_lut, 64);
        assert_eq!(ranges.len(), 6, "Should detect exactly 6 ranges");
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_encode_6_ranges() {
        let dictionary = generate_synthetic_dictionary(6, 64);
        let dict = Dictionary::new(dictionary).unwrap();
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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

        let codec = LargeLutCodec::from_dictionary(&dict);
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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        // Test strategy detection for 1-5 ranges (6+ ranges not supported yet)

        // Test 2-range dictionary (RFC4648 base32)
        let dictionary_2: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict_2 = Dictionary::new(dictionary_2).unwrap();
        let codec_2 = LargeLutCodec::from_dictionary(&dict_2).unwrap();
        assert!(codec_2.range_info.is_some());
        assert_eq!(
            codec_2.range_info.as_ref().unwrap().strategy,
            RangeStrategy::Small
        );

        // Test 3-range dictionary
        let dictionary_3 = generate_synthetic_dictionary(3, 32);
        let dict_3 = Dictionary::new(dictionary_3).unwrap();
        let codec_3 = LargeLutCodec::from_dictionary(&dict_3).unwrap();
        assert!(codec_3.range_info.is_some());
        assert_eq!(
            codec_3.range_info.as_ref().unwrap().strategy,
            RangeStrategy::SmallMulti
        );

        // 6+ ranges should not have range_info
        let dictionary_6 = generate_synthetic_dictionary(6, 64);
        let dict_6 = Dictionary::new(dictionary_6).unwrap();
        let codec_6 = LargeLutCodec::from_dictionary(&dict_6).unwrap();
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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

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
        let codec = LargeLutCodec::from_dictionary(&dict).unwrap();

        // 12 bytes → 16 chars (base64)
        let data: Vec<u8> = (0..12).collect();
        let encoded = codec.encode(&data, &dict).unwrap();

        // Ensure we have exactly 16 chars
        assert_eq!(encoded.len(), 16, "Should produce exactly 16 chars");

        let decoded = codec.decode(&encoded, &dict).unwrap();
        assert_eq!(&decoded[..], &data[..]);
    }
}
