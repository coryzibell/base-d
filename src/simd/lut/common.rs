//! Shared types for LUT-based SIMD codecs

/// Character range in dictionary (contiguous ASCII sequence)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CharRange {
    pub(super) start_idx: u8,  // First index in this range
    pub(super) end_idx: u8,    // Last index (inclusive)
    pub(super) start_char: u8, // First ASCII character
    pub(super) offset: i8,     // char = index + offset
}

/// Range-reduction strategy based on number of ranges
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) enum RangeStrategy {
    Simple,     // 1 range, direct offset
    Small,      // 2 ranges, single subs
    SmallMulti, // 3-5 ranges, subs + 1 cmp + blend
    Medium,     // 6-8 ranges, 2-3 thresholds
    Large,      // 9-12 ranges, 3-4 thresholds
    VeryLarge,  // 13-16 ranges, 4-5 thresholds
}

/// Range-reduction metadata for SSE/AVX2 encoding
#[cfg(target_arch = "x86_64")]
#[derive(Debug, Clone)]
pub(super) struct RangeInfo {
    pub(super) ranges: Vec<CharRange>,
    pub(super) offset_lut: [i8; 16],    // pshufb lookup table
    pub(super) strategy: RangeStrategy, // Encoding strategy

    // Single threshold (for 2 ranges)
    pub(super) subs_threshold: u8,       // For _mm_subs_epu8
    pub(super) cmp_value: Option<u8>,    // For _mm_cmplt_epi8 (if needed)
    pub(super) override_val: Option<u8>, // For _mm_blendv_epi8 (if needed)

    // Multiple thresholds (for 6-16 ranges)
    pub(super) thresholds: Vec<u8>, // Ordered thresholds for binary tree
    pub(super) cmp_values: Vec<u8>, // Comparison values for each threshold
}

#[cfg(target_arch = "x86_64")]
impl RangeInfo {
    /// Build range-reduction metadata for 1-5 contiguous ranges
    ///
    /// Note: 6+ range support is not implemented (would need multi-threshold
    /// compression which doesn't fit in 16-byte LUT). Falls back to scalar.
    pub(super) fn build_multi_range(ranges: &[CharRange]) -> Option<Self> {
        let num_ranges = ranges.len();

        // Reject if >5 ranges (6+ range multi-threshold not implemented)
        // Also reject if 0 ranges
        if num_ranges == 0 || num_ranges > 5 {
            return None;
        }

        // Dispatch based on range count
        match num_ranges {
            1 => Self::build_single_range(ranges),
            2 => Self::build_two_ranges(ranges),
            3..=5 => Self::build_small_multirange(ranges),
            _ => None,
        }
    }

    /// Build for single range (no reduction needed)
    fn build_single_range(ranges: &[CharRange]) -> Option<Self> {
        let range = ranges[0];

        // Single contiguous range: just use offset directly
        let mut offset_lut = [0i8; 16];
        offset_lut.iter_mut().for_each(|val| *val = range.offset);

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
        offset_lut
            .iter_mut()
            .skip(1)
            .take(range1_compressed_len)
            .for_each(|val| *val = range1.offset);

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
    ///
    /// Uses pshufb for index-to-offset lookup, which only supports 16 entries.
    /// This limits which dictionaries can use this path.
    fn build_small_multirange(ranges: &[CharRange]) -> Option<Self> {
        // Strategy: Use boundary of second-largest range as threshold
        // This maps the two largest ranges to 0, distinguish via comparison

        // Find two largest ranges by length
        let mut sorted_ranges: Vec<_> = ranges.iter().enumerate().collect();
        sorted_ranges
            .sort_by_key(|(_, r)| std::cmp::Reverse((r.end_idx - r.start_idx + 1) as usize));

        let (largest_idx, largest_range) = sorted_ranges[0];
        let (second_largest_idx, second_largest_range) = sorted_ranges[1];

        // Threshold: end of second-largest range
        // For base64: ranges[1] ('a'-'z', indices 26-51) is second-largest
        let subs_threshold = second_largest_range.end_idx;

        // Validate: after subtracting threshold, all indices must fit in 16-entry LUT
        // max_compressed_idx = last_dict_idx - threshold
        let last_dict_idx = ranges.last()?.end_idx;
        let max_compressed_idx = last_dict_idx.saturating_sub(subs_threshold);
        if max_compressed_idx >= 16 {
            // Dictionary doesn't fit in 16-byte pshufb LUT
            // Example: geohash has threshold=9, last_idx=31, compressed=22 > 16
            return None;
        }

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
                let fill_count = range_len.min(16 - compressed_idx);
                offset_lut
                    .iter_mut()
                    .skip(compressed_idx)
                    .take(fill_count)
                    .for_each(|val| *val = range.offset);
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
    #[allow(dead_code)]
    fn build_hierarchical_lut_medium(ranges: &[CharRange], _thresholds: &[u8]) -> [i8; 16] {
        let mut lut = [0i8; 16];
        let mut compressed_idx = 0usize;

        // Assign compressed indices by traversing ranges
        for range in ranges {
            let range_len = (range.end_idx - range.start_idx + 1) as usize;

            let fill_count = range_len.min(16 - compressed_idx);
            lut.iter_mut()
                .skip(compressed_idx)
                .take(fill_count)
                .for_each(|val| *val = range.offset);
            compressed_idx += range_len;
            if compressed_idx >= 16 {
                break;
            }
        }

        lut
    }

    /// Build for 9-12 ranges (large multirange with 3-4 thresholds)
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
    #[allow(dead_code)]
    fn build_hierarchical_lut_large(ranges: &[CharRange], _thresholds: &[u8]) -> [i8; 16] {
        let mut lut = [0i8; 16];
        let mut compressed_idx = 0usize;

        for range in ranges {
            let range_len = (range.end_idx - range.start_idx + 1) as usize;

            let fill_count = range_len.min(16 - compressed_idx);
            lut.iter_mut()
                .skip(compressed_idx)
                .take(fill_count)
                .for_each(|val| *val = range.offset);
            compressed_idx += range_len;
            if compressed_idx >= 16 {
                break;
            }
        }

        lut
    }

    /// Build for 13-16 ranges (very large multirange with 4-5 thresholds)
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
    #[allow(dead_code)]
    fn build_hierarchical_lut_very_large(ranges: &[CharRange], _thresholds: &[u8]) -> [i8; 16] {
        let mut lut = [0i8; 16];
        let mut compressed_idx = 0usize;

        for range in ranges {
            let range_len = (range.end_idx - range.start_idx + 1) as usize;

            let fill_count = range_len.min(16 - compressed_idx);
            lut.iter_mut()
                .skip(compressed_idx)
                .take(fill_count)
                .for_each(|val| *val = range.offset);
            compressed_idx += range_len;
            if compressed_idx >= 16 {
                break;
            }
        }

        lut
    }
}
