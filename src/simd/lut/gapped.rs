//! GappedSequentialCodec: SIMD codec for near-sequential dictionaries with gaps
//!
//! This codec handles dictionaries that are mostly sequential but have a few
//! missing characters (gaps). Examples:
//! - Geohash: 0-9, b-h, j-k, m-n, p-z (missing a, i, l, o)
//! - Crockford Base32: 0-9, A-H, J-K, M-N, P-T, V-Z (missing I, L, O, U)
//!
//! Strategy: Instead of a lookup table, use threshold comparisons to compute
//! the character offset. For each gap, indices after it need +1 adjustment.
//!
//! char = index + base_offset + sum(index >= threshold[i])
//!
//! This requires O(gaps) SIMD comparisons, which is efficient for ≤8 gaps.

use crate::core::dictionary::Dictionary;

/// Maximum number of gaps supported (more gaps = more SIMD instructions)
const MAX_GAPS: usize = 8;

/// Metadata for a gapped sequential dictionary
#[derive(Debug, Clone)]
pub struct GapInfo {
    /// Base offset: first_char - 0
    pub base_offset: u8,
    /// Thresholds where gaps occur (sorted)
    /// threshold[i] = first index AFTER gap i
    pub thresholds: Vec<u8>,
    /// Additional offset for indices >= each threshold
    /// Usually 1 for single-char gaps, but could be more
    pub adjustments: Vec<u8>,
}

/// SIMD codec for gapped sequential dictionaries
pub struct GappedSequentialCodec {
    gap_info: GapInfo,
    bits_per_symbol: u8,
    /// Decoding LUT: char -> index (256 bytes, sparse)
    decode_lut: [u8; 256],
}

impl GappedSequentialCodec {
    /// Analyze dictionary and create codec if it's a gapped sequential pattern
    ///
    /// Returns None if:
    /// - Dictionary is not power-of-2 base
    /// - Dictionary has too many gaps (> MAX_GAPS)
    /// - Dictionary is truly arbitrary (not near-sequential)
    pub fn from_dictionary(dict: &Dictionary) -> Option<Self> {
        let base = dict.base();

        // Must be power of 2
        if !base.is_power_of_two() {
            return None;
        }

        let bits_per_symbol = (base as f64).log2() as u8;

        // Only support 4, 5, 6, 8 bits (base 16, 32, 64, 256)
        if !matches!(bits_per_symbol, 4 | 5 | 6 | 8) {
            return None;
        }

        // Analyze the dictionary for gaps
        let gap_info = Self::analyze_gaps(dict)?;

        // Build decode LUT
        let mut decode_lut = [0xFFu8; 256];
        for i in 0..base {
            if let Some(ch) = dict.encode_digit(i)
                && (ch as u32) < 256
            {
                decode_lut[ch as usize] = i as u8;
            }
        }

        Some(Self {
            gap_info,
            bits_per_symbol,
            decode_lut,
        })
    }

    /// Analyze dictionary to find gap pattern
    fn analyze_gaps(dict: &Dictionary) -> Option<GapInfo> {
        let base = dict.base();

        // Get all characters
        let chars: Vec<char> = (0..base).filter_map(|i| dict.encode_digit(i)).collect();
        if chars.len() != base {
            return None;
        }

        // All chars must be ASCII
        if chars.iter().any(|&c| (c as u32) > 127) {
            return None;
        }

        let first_char = chars[0] as u8;
        let base_offset = first_char;

        // Compute expected char if sequential, find gaps
        let mut thresholds = Vec::new();
        let mut adjustments = Vec::new();
        let mut cumulative_gap = 0u8;

        for (i, &ch) in chars.iter().enumerate() {
            let expected = first_char
                .wrapping_add(i as u8)
                .wrapping_add(cumulative_gap);
            let actual = ch as u8;

            if actual != expected {
                // Found a gap
                let gap_size = actual.wrapping_sub(expected);
                cumulative_gap = cumulative_gap.wrapping_add(gap_size);
                thresholds.push(i as u8);
                adjustments.push(gap_size);
            }
        }

        // Too many gaps?
        if thresholds.len() > MAX_GAPS {
            return None;
        }

        // No gaps = pure sequential, should use GenericSimdCodec instead
        if thresholds.is_empty() {
            return None;
        }

        Some(GapInfo {
            base_offset,
            thresholds,
            adjustments,
        })
    }

    /// Encode data using SIMD threshold comparisons
    pub fn encode(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        if data.is_empty() {
            return Some(String::new());
        }

        // Calculate output length
        let output_len = (data.len() * 8).div_ceil(self.bits_per_symbol as usize);
        let mut result = String::with_capacity(output_len);

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("ssse3") {
                unsafe {
                    self.encode_ssse3(data, &mut result);
                }
                return Some(result);
            }
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            self.encode_neon(data, &mut result);
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            // Scalar fallback (only for non-SIMD paths on x86 or other architectures)
            self.encode_scalar(data, &mut result);
        }

        Some(result)
    }

    /// Scalar encoding (fallback)
    fn encode_scalar(&self, data: &[u8], result: &mut String) {
        let bits_per_char = self.bits_per_symbol as usize;
        let mask = (1u32 << bits_per_char) - 1;

        let mut bit_buffer = 0u32;
        let mut bits_in_buffer = 0usize;

        for &byte in data {
            bit_buffer = (bit_buffer << 8) | (byte as u32);
            bits_in_buffer += 8;

            while bits_in_buffer >= bits_per_char {
                bits_in_buffer -= bits_per_char;
                let index = ((bit_buffer >> bits_in_buffer) & mask) as u8;
                let ch = self.index_to_char(index);
                result.push(ch as char);
            }
        }

        // Handle remaining bits
        if bits_in_buffer > 0 {
            let index = ((bit_buffer << (bits_per_char - bits_in_buffer)) & mask) as u8;
            let ch = self.index_to_char(index);
            result.push(ch as char);
        }
    }

    /// Convert index to character using threshold method
    #[inline]
    fn index_to_char(&self, index: u8) -> u8 {
        let mut ch = self.gap_info.base_offset.wrapping_add(index);
        for (threshold, adjustment) in self
            .gap_info
            .thresholds
            .iter()
            .zip(&self.gap_info.adjustments)
        {
            if index >= *threshold {
                ch = ch.wrapping_add(*adjustment);
            }
        }
        ch
    }

    /// SSSE3 encoding implementation
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3(&self, data: &[u8], result: &mut String) {
        // Safe: pattern matching and function dispatch
        match self.bits_per_symbol {
            5 => unsafe { self.encode_ssse3_5bit(data, result) },
            6 => unsafe { self.encode_ssse3_6bit(data, result) },
            4 => unsafe { self.encode_ssse3_4bit(data, result) },
            _ => self.encode_scalar(data, result),
        }
    }

    /// SSSE3 5-bit encoding (base32)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3_5bit(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        // Safe: constant definition and bounds check
        const BLOCK_SIZE: usize = 5; // 5 bytes -> 8 chars

        if data.len() < BLOCK_SIZE {
            self.encode_scalar(data, result);
            return;
        }

        // Unsafe: SIMD vector initialization (target_feature makes these safe to call)
        let threshold_vecs: Vec<__m128i> = self
            .gap_info
            .thresholds
            .iter()
            .map(|&t| _mm_set1_epi8((t.wrapping_sub(1)) as i8)) // cmpgt needs t-1
            .collect();

        let adjustment_vecs: Vec<__m128i> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| _mm_set1_epi8(a as i8))
            .collect();

        let base_offset_vec = _mm_set1_epi8(self.gap_info.base_offset as i8);

        // Safe: arithmetic
        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Safe: debug assertion
            debug_assert!(
                offset + 5 <= data.len(),
                "SIMD bounds check: offset {} + 5 exceeds len {}",
                offset,
                data.len()
            );

            // Unsafe: unchecked indexing
            let (b0, b1, b2, b3, b4) = unsafe {
                (
                    *data.get_unchecked(offset),
                    *data.get_unchecked(offset + 1),
                    *data.get_unchecked(offset + 2),
                    *data.get_unchecked(offset + 3),
                    *data.get_unchecked(offset + 4),
                )
            };

            // Safe: bit manipulation
            let mut indices = [0u8; 16];
            indices[0] = (b0 >> 3) & 0x1F;
            indices[1] = ((b0 << 2) | (b1 >> 6)) & 0x1F;
            indices[2] = (b1 >> 1) & 0x1F;
            indices[3] = ((b1 << 4) | (b2 >> 4)) & 0x1F;
            indices[4] = ((b2 << 1) | (b3 >> 7)) & 0x1F;
            indices[5] = (b3 >> 2) & 0x1F;
            indices[6] = ((b3 << 3) | (b4 >> 5)) & 0x1F;
            indices[7] = b4 & 0x1F;

            // Unsafe: SIMD load operation (pointer dereference)
            let idx_vec = unsafe { _mm_loadu_si128(indices.as_ptr() as *const __m128i) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut char_vec = _mm_add_epi8(base_offset_vec, idx_vec);

            // Add adjustment for each threshold
            for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                // cmpgt returns 0xFF where idx > threshold-1, i.e., idx >= threshold
                let mask = _mm_cmpgt_epi8(idx_vec, *thresh_vec);
                // AND with adjustment to get conditional add
                let adj = _mm_and_si128(mask, *adj_vec);
                char_vec = _mm_add_epi8(char_vec, adj);
            }

            // Unsafe: SIMD store
            let mut output = [0u8; 16];
            unsafe {
                _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, char_vec);
            }

            // Safe: iteration and push
            for &ch in &output[..8] {
                result.push(ch as char);
            }

            offset += BLOCK_SIZE;
        }

        // Safe: remainder handling
        if simd_bytes < data.len() {
            self.encode_scalar(&data[simd_bytes..], result);
        }
    }

    /// SSSE3 6-bit encoding (base64)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3_6bit(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        // Safe: constant definition and bounds check
        const BLOCK_SIZE: usize = 3; // 3 bytes -> 4 chars

        if data.len() < BLOCK_SIZE {
            self.encode_scalar(data, result);
            return;
        }

        // Unsafe: SIMD vector initialization (target_feature makes these safe to call)
        let threshold_vecs: Vec<__m128i> = self
            .gap_info
            .thresholds
            .iter()
            .map(|&t| _mm_set1_epi8((t.wrapping_sub(1)) as i8))
            .collect();

        let adjustment_vecs: Vec<__m128i> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| _mm_set1_epi8(a as i8))
            .collect();

        let base_offset_vec = _mm_set1_epi8(self.gap_info.base_offset as i8);

        // Safe: arithmetic
        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Safe: debug assertion
            debug_assert!(
                offset + 3 <= data.len(),
                "SIMD bounds check: offset {} + 3 exceeds len {}",
                offset,
                data.len()
            );

            // Unsafe: unchecked indexing
            let (b0, b1, b2) = unsafe {
                (
                    *data.get_unchecked(offset),
                    *data.get_unchecked(offset + 1),
                    *data.get_unchecked(offset + 2),
                )
            };

            // Safe: bit manipulation
            let mut indices = [0u8; 16];
            indices[0] = (b0 >> 2) & 0x3F;
            indices[1] = ((b0 << 4) | (b1 >> 4)) & 0x3F;
            indices[2] = ((b1 << 2) | (b2 >> 6)) & 0x3F;
            indices[3] = b2 & 0x3F;

            // Unsafe: SIMD load operation (pointer dereference)
            let idx_vec = unsafe { _mm_loadu_si128(indices.as_ptr() as *const __m128i) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut char_vec = _mm_add_epi8(base_offset_vec, idx_vec);

            for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                let mask = _mm_cmpgt_epi8(idx_vec, *thresh_vec);
                let adj = _mm_and_si128(mask, *adj_vec);
                char_vec = _mm_add_epi8(char_vec, adj);
            }

            // Unsafe: SIMD store
            let mut output = [0u8; 16];
            unsafe {
                _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, char_vec);
            }

            // Safe: iteration and push
            for &ch in &output[..4] {
                result.push(ch as char);
            }

            offset += BLOCK_SIZE;
        }

        // Safe: remainder handling
        if simd_bytes < data.len() {
            self.encode_scalar(&data[simd_bytes..], result);
        }
    }

    /// SSSE3 4-bit encoding (base16)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3_4bit(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        // Safe: constant definition and bounds check
        const BLOCK_SIZE: usize = 8; // 8 bytes -> 16 chars

        if data.len() < BLOCK_SIZE {
            self.encode_scalar(data, result);
            return;
        }

        // Unsafe: SIMD vector initialization (target_feature makes these safe to call)
        let threshold_vecs: Vec<__m128i> = self
            .gap_info
            .thresholds
            .iter()
            .map(|&t| _mm_set1_epi8((t.wrapping_sub(1)) as i8))
            .collect();

        let adjustment_vecs: Vec<__m128i> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| _mm_set1_epi8(a as i8))
            .collect();

        let base_offset_vec = _mm_set1_epi8(self.gap_info.base_offset as i8);

        // Safe: arithmetic
        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Safe: debug assertion
            debug_assert!(
                offset + 8 <= data.len(),
                "SIMD bounds check: offset {} + 8 exceeds len {}",
                offset,
                data.len()
            );

            // Safe: bit manipulation with unsafe unchecked indexing
            let mut indices = [0u8; 16];
            for i in 0..8 {
                let byte = unsafe { *data.get_unchecked(offset + i) };
                indices[i * 2] = (byte >> 4) & 0x0F;
                indices[i * 2 + 1] = byte & 0x0F;
            }

            // Unsafe: SIMD load operation (pointer dereference)
            let idx_vec = unsafe { _mm_loadu_si128(indices.as_ptr() as *const __m128i) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut char_vec = _mm_add_epi8(base_offset_vec, idx_vec);

            for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                let mask = _mm_cmpgt_epi8(idx_vec, *thresh_vec);
                let adj = _mm_and_si128(mask, *adj_vec);
                char_vec = _mm_add_epi8(char_vec, adj);
            }

            // Unsafe: SIMD store
            let mut output = [0u8; 16];
            unsafe {
                _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, char_vec);
            }

            // Safe: iteration and push
            for &ch in &output[..16] {
                result.push(ch as char);
            }

            offset += BLOCK_SIZE;
        }

        // Safe: remainder handling
        if simd_bytes < data.len() {
            self.encode_scalar(&data[simd_bytes..], result);
        }
    }

    /// NEON encoding implementation
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon(&self, data: &[u8], result: &mut String) {
        // Safe: pattern matching and function dispatch
        match self.bits_per_symbol {
            5 => unsafe { self.encode_neon_5bit(data, result) },
            6 => unsafe { self.encode_neon_6bit(data, result) },
            4 => unsafe { self.encode_neon_4bit(data, result) },
            _ => self.encode_scalar(data, result),
        }
    }

    /// NEON 5-bit encoding (base32)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_5bit(&self, data: &[u8], result: &mut String) {
        use std::arch::aarch64::*;

        // Safe: constant definition and bounds check
        const BLOCK_SIZE: usize = 5;

        if data.len() < BLOCK_SIZE {
            self.encode_scalar(data, result);
            return;
        }

        // Unsafe: SIMD vector initialization (target_feature makes these safe to call)
        let threshold_vecs: Vec<uint8x16_t> = self
            .gap_info
            .thresholds
            .iter()
            .map(|&t| vdupq_n_u8(t))
            .collect();

        let adjustment_vecs: Vec<uint8x16_t> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| vdupq_n_u8(a))
            .collect();

        let base_offset_vec = vdupq_n_u8(self.gap_info.base_offset);

        // Safe: arithmetic
        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Safe: debug assertion
            debug_assert!(
                offset + 5 <= data.len(),
                "SIMD bounds check: offset {} + 5 exceeds len {}",
                offset,
                data.len()
            );

            // Unsafe: unchecked indexing
            let (b0, b1, b2, b3, b4) = unsafe {
                (
                    *data.get_unchecked(offset),
                    *data.get_unchecked(offset + 1),
                    *data.get_unchecked(offset + 2),
                    *data.get_unchecked(offset + 3),
                    *data.get_unchecked(offset + 4),
                )
            };

            // Safe: bit manipulation
            let mut indices = [0u8; 16];
            indices[0] = (b0 >> 3) & 0x1F;
            indices[1] = ((b0 << 2) | (b1 >> 6)) & 0x1F;
            indices[2] = (b1 >> 1) & 0x1F;
            indices[3] = ((b1 << 4) | (b2 >> 4)) & 0x1F;
            indices[4] = ((b2 << 1) | (b3 >> 7)) & 0x1F;
            indices[5] = (b3 >> 2) & 0x1F;
            indices[6] = ((b3 << 3) | (b4 >> 5)) & 0x1F;
            indices[7] = b4 & 0x1F;

            // Unsafe: SIMD load operation (pointer dereference)
            let idx_vec = unsafe { vld1q_u8(indices.as_ptr()) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut char_vec = vaddq_u8(base_offset_vec, idx_vec);

            for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                // vcgeq returns 0xFF where idx >= threshold
                let mask = vcgeq_u8(idx_vec, *thresh_vec);
                let adj = vandq_u8(mask, *adj_vec);
                char_vec = vaddq_u8(char_vec, adj);
            }

            // Unsafe: SIMD store
            let mut output = [0u8; 16];
            unsafe {
                vst1q_u8(output.as_mut_ptr(), char_vec);
            }

            // Safe: iteration and push
            for &ch in &output[..8] {
                result.push(ch as char);
            }

            offset += BLOCK_SIZE;
        }

        // Safe: remainder handling
        if simd_bytes < data.len() {
            self.encode_scalar(&data[simd_bytes..], result);
        }
    }

    /// NEON 6-bit encoding (base64)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_6bit(&self, data: &[u8], result: &mut String) {
        use std::arch::aarch64::*;

        // Safe: constant definition and bounds check
        const BLOCK_SIZE: usize = 3;

        if data.len() < BLOCK_SIZE {
            self.encode_scalar(data, result);
            return;
        }

        // Unsafe: SIMD vector initialization (target_feature makes these safe to call)
        let threshold_vecs: Vec<uint8x16_t> = self
            .gap_info
            .thresholds
            .iter()
            .map(|&t| vdupq_n_u8(t))
            .collect();

        let adjustment_vecs: Vec<uint8x16_t> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| vdupq_n_u8(a))
            .collect();

        let base_offset_vec = vdupq_n_u8(self.gap_info.base_offset);

        // Safe: arithmetic
        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Safe: debug assertion
            debug_assert!(
                offset + 3 <= data.len(),
                "SIMD bounds check: offset {} + 3 exceeds len {}",
                offset,
                data.len()
            );

            // Unsafe: unchecked indexing
            let (b0, b1, b2) = unsafe {
                (
                    *data.get_unchecked(offset),
                    *data.get_unchecked(offset + 1),
                    *data.get_unchecked(offset + 2),
                )
            };

            // Safe: bit manipulation
            let mut indices = [0u8; 16];
            indices[0] = (b0 >> 2) & 0x3F;
            indices[1] = ((b0 << 4) | (b1 >> 4)) & 0x3F;
            indices[2] = ((b1 << 2) | (b2 >> 6)) & 0x3F;
            indices[3] = b2 & 0x3F;

            // Unsafe: SIMD load operation (pointer dereference)
            let idx_vec = unsafe { vld1q_u8(indices.as_ptr()) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut char_vec = vaddq_u8(base_offset_vec, idx_vec);

            for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                let mask = vcgeq_u8(idx_vec, *thresh_vec);
                let adj = vandq_u8(mask, *adj_vec);
                char_vec = vaddq_u8(char_vec, adj);
            }

            // Unsafe: SIMD store
            let mut output = [0u8; 16];
            unsafe {
                vst1q_u8(output.as_mut_ptr(), char_vec);
            }

            // Safe: iteration and push
            for &ch in &output[..4] {
                result.push(ch as char);
            }

            offset += BLOCK_SIZE;
        }

        // Safe: remainder handling
        if simd_bytes < data.len() {
            self.encode_scalar(&data[simd_bytes..], result);
        }
    }

    /// NEON 4-bit encoding (base16)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_4bit(&self, data: &[u8], result: &mut String) {
        use std::arch::aarch64::*;

        // Safe: constant definition and bounds check
        const BLOCK_SIZE: usize = 8;

        if data.len() < BLOCK_SIZE {
            self.encode_scalar(data, result);
            return;
        }

        // Unsafe: SIMD vector initialization (target_feature makes these safe to call)
        let threshold_vecs: Vec<uint8x16_t> = self
            .gap_info
            .thresholds
            .iter()
            .map(|&t| vdupq_n_u8(t))
            .collect();

        let adjustment_vecs: Vec<uint8x16_t> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| vdupq_n_u8(a))
            .collect();

        let base_offset_vec = vdupq_n_u8(self.gap_info.base_offset);

        // Safe: arithmetic
        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Safe: debug assertion
            debug_assert!(
                offset + 8 <= data.len(),
                "SIMD bounds check: offset {} + 8 exceeds len {}",
                offset,
                data.len()
            );

            // Safe: bit manipulation with unsafe unchecked indexing
            let mut indices = [0u8; 16];
            for i in 0..8 {
                let byte = unsafe { *data.get_unchecked(offset + i) };
                indices[i * 2] = (byte >> 4) & 0x0F;
                indices[i * 2 + 1] = byte & 0x0F;
            }

            // Unsafe: SIMD load operation (pointer dereference)
            let idx_vec = unsafe { vld1q_u8(indices.as_ptr()) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut char_vec = vaddq_u8(base_offset_vec, idx_vec);

            for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                let mask = vcgeq_u8(idx_vec, *thresh_vec);
                let adj = vandq_u8(mask, *adj_vec);
                char_vec = vaddq_u8(char_vec, adj);
            }

            // Unsafe: SIMD store
            let mut output = [0u8; 16];
            unsafe {
                vst1q_u8(output.as_mut_ptr(), char_vec);
            }

            // Safe: iteration and push
            for &ch in &output[..16] {
                result.push(ch as char);
            }

            offset += BLOCK_SIZE;
        }

        // Safe: remainder handling
        if simd_bytes < data.len() {
            self.encode_scalar(&data[simd_bytes..], result);
        }
    }

    /// Decode encoded string back to bytes
    pub fn decode(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        if encoded.is_empty() {
            return Some(Vec::new());
        }

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("ssse3") {
                return unsafe { self.decode_ssse3(encoded) };
            }
            self.decode_scalar(encoded)
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            self.decode_neon(encoded)
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            self.decode_scalar(encoded)
        }
    }

    /// Scalar decoding (fallback)
    fn decode_scalar(&self, encoded: &str) -> Option<Vec<u8>> {
        let bits_per_char = self.bits_per_symbol as usize;

        // Estimate output size
        let estimated_len = (encoded.len() * bits_per_char) / 8;
        let mut result = Vec::with_capacity(estimated_len);

        let mut bit_buffer = 0u32;
        let mut bits_in_buffer = 0usize;

        for ch in encoded.chars() {
            if (ch as u32) >= 256 {
                return None; // Invalid character
            }

            let index = self.decode_lut[ch as usize];
            if index == 0xFF {
                return None; // Invalid character
            }

            bit_buffer = (bit_buffer << bits_per_char) | (index as u32);
            bits_in_buffer += bits_per_char;

            while bits_in_buffer >= 8 {
                bits_in_buffer -= 8;
                result.push(((bit_buffer >> bits_in_buffer) & 0xFF) as u8);
            }
        }

        Some(result)
    }

    /// SSSE3 decoding implementation
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_ssse3(&self, encoded: &str) -> Option<Vec<u8>> {
        match self.bits_per_symbol {
            5 => unsafe { self.decode_ssse3_5bit(encoded) },
            6 => unsafe { self.decode_ssse3_6bit(encoded) },
            4 => unsafe { self.decode_ssse3_4bit(encoded) },
            _ => self.decode_scalar(encoded),
        }
    }

    /// SSSE3 5-bit decoding (base32: 8 chars -> 5 bytes)
    /// Uses SIMD for parallel char-to-index conversion via inverse threshold method
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_ssse3_5bit(&self, encoded: &str) -> Option<Vec<u8>> {
        use std::arch::x86_64::*;

        // Safe: constants, arithmetic, slice operations
        const INPUT_BLOCK: usize = 8;

        let encoded_bytes = encoded.as_bytes();
        let estimated_len = (encoded_bytes.len() * 5) / 8;
        let mut result = Vec::with_capacity(estimated_len);

        let num_blocks = encoded_bytes.len() / INPUT_BLOCK;
        let simd_chars = num_blocks * INPUT_BLOCK;

        // SIMD vector initialization (safe in target_feature context)
        // Pre-compute SIMD vectors for inverse threshold method
        // For decode: index = char - base_offset - sum(char > adjusted_threshold[i])
        let base_offset_vec = _mm_set1_epi8(self.gap_info.base_offset as i8);

        // Threshold computation logic
        // Build adjusted thresholds: the char value where each gap starts
        let mut cumulative = 0u8;
        let adjusted_thresholds: Vec<__m128i> = self
            .gap_info
            .thresholds
            .iter()
            .zip(&self.gap_info.adjustments)
            .map(|(&t, &a)| {
                let thresh = self
                    .gap_info
                    .base_offset
                    .wrapping_add(t)
                    .wrapping_add(cumulative);
                cumulative = cumulative.wrapping_add(a);
                _mm_set1_epi8(thresh.wrapping_sub(1) as i8) // cmpgt needs thresh-1
            })
            .collect();

        let adjustment_vecs: Vec<__m128i> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| _mm_set1_epi8(a as i8))
            .collect();

        let max_valid = _mm_set1_epi8(31);

        for block in 0..num_blocks {
            // Safe: offset calculation
            let offset = block * INPUT_BLOCK;

            // Safe: array init and copy
            let mut chars = [0u8; 16];
            chars[..8].copy_from_slice(&encoded_bytes[offset..offset + INPUT_BLOCK]);

            // Unsafe: SIMD load (pointer dereference)
            let char_vec = unsafe { _mm_loadu_si128(chars.as_ptr() as *const __m128i) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut idx_vec = _mm_sub_epi8(char_vec, base_offset_vec);

            // Subtract adjustment for each threshold the char exceeds
            for (thresh_vec, adj_vec) in adjusted_thresholds.iter().zip(&adjustment_vecs) {
                // cmpgt returns 0xFF where char > threshold-1, i.e., char >= threshold
                let mask = _mm_cmpgt_epi8(char_vec, *thresh_vec);
                let adj = _mm_and_si128(mask, *adj_vec);
                idx_vec = _mm_sub_epi8(idx_vec, adj);
            }

            // SIMD validation
            let invalid_mask = _mm_cmpgt_epi8(idx_vec, max_valid);
            let any_invalid = _mm_movemask_epi8(invalid_mask);

            // Safe: conditional check
            if (any_invalid & 0xFF) != 0 {
                return self.decode_scalar(encoded);
            }

            // Safe: array init
            let mut indices = [0u8; 16];
            // Unsafe: SIMD store
            unsafe { _mm_storeu_si128(indices.as_mut_ptr() as *mut __m128i, idx_vec) };

            // Safe: bit packing and push
            result.push((indices[0] << 3) | (indices[1] >> 2));
            result.push((indices[1] << 6) | (indices[2] << 1) | (indices[3] >> 4));
            result.push((indices[3] << 4) | (indices[4] >> 1));
            result.push((indices[4] << 7) | (indices[5] << 2) | (indices[6] >> 3));
            result.push((indices[6] << 5) | indices[7]);
        }

        // Safe: remainder handling (no SIMD)
        if simd_chars < encoded_bytes.len() {
            let remainder = &encoded[simd_chars..];
            let bits_per_char = 5usize;
            let mut bit_buffer = 0u32;
            let mut bits_in_buffer = 0usize;

            for ch in remainder.chars() {
                if (ch as u32) >= 256 {
                    return None;
                }
                let index = self.decode_lut[ch as usize];
                if index == 0xFF {
                    return None;
                }
                bit_buffer = (bit_buffer << bits_per_char) | (index as u32);
                bits_in_buffer += bits_per_char;

                while bits_in_buffer >= 8 {
                    bits_in_buffer -= 8;
                    result.push(((bit_buffer >> bits_in_buffer) & 0xFF) as u8);
                }
            }
        }

        Some(result)
    }

    /// SSSE3 6-bit decoding (base64: 4 chars -> 3 bytes)
    /// Uses SIMD for parallel char-to-index conversion via inverse threshold method
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_ssse3_6bit(&self, encoded: &str) -> Option<Vec<u8>> {
        use std::arch::x86_64::*;

        // Safe: constants, arithmetic, slice operations
        const INPUT_BLOCK: usize = 4;

        let encoded_bytes = encoded.as_bytes();
        let estimated_len = (encoded_bytes.len() * 3) / 4;
        let mut result = Vec::with_capacity(estimated_len);

        let num_blocks = encoded_bytes.len() / INPUT_BLOCK;
        let simd_chars = num_blocks * INPUT_BLOCK;

        // SIMD vector initialization (safe in target_feature context)
        let base_offset_vec = _mm_set1_epi8(self.gap_info.base_offset as i8);

        // Threshold computation logic
        let mut cumulative = 0u8;
        let adjusted_thresholds: Vec<__m128i> = self
            .gap_info
            .thresholds
            .iter()
            .zip(&self.gap_info.adjustments)
            .map(|(&t, &a)| {
                let thresh = self
                    .gap_info
                    .base_offset
                    .wrapping_add(t)
                    .wrapping_add(cumulative);
                cumulative = cumulative.wrapping_add(a);
                _mm_set1_epi8(thresh.wrapping_sub(1) as i8)
            })
            .collect();

        let adjustment_vecs: Vec<__m128i> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| _mm_set1_epi8(a as i8))
            .collect();

        let max_valid = _mm_set1_epi8(63);

        for block in 0..num_blocks {
            // Safe: offset calculation
            let offset = block * INPUT_BLOCK;

            // Safe: array init and copy
            let mut chars = [0u8; 16];
            chars[..4].copy_from_slice(&encoded_bytes[offset..offset + INPUT_BLOCK]);

            // Unsafe: SIMD load (pointer dereference)
            let char_vec = unsafe { _mm_loadu_si128(chars.as_ptr() as *const __m128i) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut idx_vec = _mm_sub_epi8(char_vec, base_offset_vec);

            // Subtract adjustment for each threshold the char exceeds
            for (thresh_vec, adj_vec) in adjusted_thresholds.iter().zip(&adjustment_vecs) {
                let mask = _mm_cmpgt_epi8(char_vec, *thresh_vec);
                let adj = _mm_and_si128(mask, *adj_vec);
                idx_vec = _mm_sub_epi8(idx_vec, adj);
            }

            // SIMD validation
            let invalid_mask = _mm_cmpgt_epi8(idx_vec, max_valid);
            let any_invalid = _mm_movemask_epi8(invalid_mask);

            // Safe: conditional check
            if (any_invalid & 0xF) != 0 {
                return self.decode_scalar(encoded);
            }

            // Safe: array init
            let mut indices = [0u8; 16];
            // Unsafe: SIMD store
            unsafe { _mm_storeu_si128(indices.as_mut_ptr() as *mut __m128i, idx_vec) };

            // Safe: bit packing and push
            result.push((indices[0] << 2) | (indices[1] >> 4));
            result.push((indices[1] << 4) | (indices[2] >> 2));
            result.push((indices[2] << 6) | indices[3]);
        }

        // Safe: remainder handling (no SIMD)
        if simd_chars < encoded_bytes.len() {
            let remainder = &encoded[simd_chars..];
            let bits_per_char = 6usize;
            let mut bit_buffer = 0u32;
            let mut bits_in_buffer = 0usize;

            for ch in remainder.chars() {
                if (ch as u32) >= 256 {
                    return None;
                }
                let index = self.decode_lut[ch as usize];
                if index == 0xFF {
                    return None;
                }
                bit_buffer = (bit_buffer << bits_per_char) | (index as u32);
                bits_in_buffer += bits_per_char;

                while bits_in_buffer >= 8 {
                    bits_in_buffer -= 8;
                    result.push(((bit_buffer >> bits_in_buffer) & 0xFF) as u8);
                }
            }
        }

        Some(result)
    }

    /// SSSE3 4-bit decoding (base16: 16 chars -> 8 bytes)
    /// Uses SIMD for parallel char-to-index conversion via inverse threshold method
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn decode_ssse3_4bit(&self, encoded: &str) -> Option<Vec<u8>> {
        use std::arch::x86_64::*;

        // Safe: constants, arithmetic, slice operations
        const INPUT_BLOCK: usize = 16;

        let encoded_bytes = encoded.as_bytes();
        let estimated_len = encoded_bytes.len() / 2;
        let mut result = Vec::with_capacity(estimated_len);

        let num_blocks = encoded_bytes.len() / INPUT_BLOCK;
        let simd_chars = num_blocks * INPUT_BLOCK;

        // SIMD vector initialization (safe in target_feature context)
        let base_offset_vec = _mm_set1_epi8(self.gap_info.base_offset as i8);

        // Threshold computation logic
        let mut cumulative = 0u8;
        let adjusted_thresholds: Vec<__m128i> = self
            .gap_info
            .thresholds
            .iter()
            .zip(&self.gap_info.adjustments)
            .map(|(&t, &a)| {
                let thresh = self
                    .gap_info
                    .base_offset
                    .wrapping_add(t)
                    .wrapping_add(cumulative);
                cumulative = cumulative.wrapping_add(a);
                _mm_set1_epi8(thresh.wrapping_sub(1) as i8)
            })
            .collect();

        let adjustment_vecs: Vec<__m128i> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| _mm_set1_epi8(a as i8))
            .collect();

        let max_valid = _mm_set1_epi8(15);

        for block in 0..num_blocks {
            // Safe: offset calculation
            let offset = block * INPUT_BLOCK;

            // Unsafe: SIMD load (pointer dereference)
            let char_vec =
                unsafe { _mm_loadu_si128(encoded_bytes[offset..].as_ptr() as *const __m128i) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut idx_vec = _mm_sub_epi8(char_vec, base_offset_vec);

            // Subtract adjustment for each threshold the char exceeds
            for (thresh_vec, adj_vec) in adjusted_thresholds.iter().zip(&adjustment_vecs) {
                let mask = _mm_cmpgt_epi8(char_vec, *thresh_vec);
                let adj = _mm_and_si128(mask, *adj_vec);
                idx_vec = _mm_sub_epi8(idx_vec, adj);
            }

            // SIMD validation
            let invalid_mask = _mm_cmpgt_epi8(idx_vec, max_valid);
            let any_invalid = _mm_movemask_epi8(invalid_mask);

            // Safe: conditional check
            if any_invalid != 0 {
                return self.decode_scalar(encoded);
            }

            // Safe: array init
            let mut indices = [0u8; 16];
            // Unsafe: SIMD store
            unsafe { _mm_storeu_si128(indices.as_mut_ptr() as *mut __m128i, idx_vec) };

            // Safe: bit packing and push
            for i in 0..8 {
                result.push((indices[i * 2] << 4) | indices[i * 2 + 1]);
            }
        }

        // Safe: remainder handling (no SIMD)
        if simd_chars < encoded_bytes.len() {
            let remainder = &encoded[simd_chars..];
            let bits_per_char = 4usize;
            let mut bit_buffer = 0u32;
            let mut bits_in_buffer = 0usize;

            for ch in remainder.chars() {
                if (ch as u32) >= 256 {
                    return None;
                }
                let index = self.decode_lut[ch as usize];
                if index == 0xFF {
                    return None;
                }
                bit_buffer = (bit_buffer << bits_per_char) | (index as u32);
                bits_in_buffer += bits_per_char;

                while bits_in_buffer >= 8 {
                    bits_in_buffer -= 8;
                    result.push(((bit_buffer >> bits_in_buffer) & 0xFF) as u8);
                }
            }
        }

        Some(result)
    }

    /// NEON decoding implementation
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn decode_neon(&self, encoded: &str) -> Option<Vec<u8>> {
        match self.bits_per_symbol {
            5 => unsafe { self.decode_neon_5bit(encoded) },
            6 => unsafe { self.decode_neon_6bit(encoded) },
            4 => unsafe { self.decode_neon_4bit(encoded) },
            _ => self.decode_scalar(encoded),
        }
    }

    /// NEON 5-bit decoding (base32: 8 chars -> 5 bytes)
    /// Uses SIMD for parallel char-to-index conversion via inverse threshold method
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn decode_neon_5bit(&self, encoded: &str) -> Option<Vec<u8>> {
        use std::arch::aarch64::*;

        // Safe: constants, arithmetic, slice operations
        const INPUT_BLOCK: usize = 8;

        let encoded_bytes = encoded.as_bytes();
        let estimated_len = (encoded_bytes.len() * 5) / 8;
        let mut result = Vec::with_capacity(estimated_len);

        let num_blocks = encoded_bytes.len() / INPUT_BLOCK;
        let simd_chars = num_blocks * INPUT_BLOCK;

        // SIMD vector initialization (safe in target_feature context)
        // Pre-compute SIMD vectors for inverse threshold method
        // For decode: index = char - base_offset - sum(char > adjusted_threshold[i])
        let base_offset_vec = vdupq_n_u8(self.gap_info.base_offset);

        // Threshold computation logic
        // Build adjusted thresholds: the char value where each gap starts
        let mut cumulative = 0u8;
        let adjusted_thresholds: Vec<uint8x16_t> = self
            .gap_info
            .thresholds
            .iter()
            .zip(&self.gap_info.adjustments)
            .map(|(&t, &a)| {
                let thresh = self
                    .gap_info
                    .base_offset
                    .wrapping_add(t)
                    .wrapping_add(cumulative);
                cumulative = cumulative.wrapping_add(a);
                vdupq_n_u8(thresh) // vcgeq needs the threshold directly
            })
            .collect();

        let adjustment_vecs: Vec<uint8x16_t> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| vdupq_n_u8(a))
            .collect();

        let max_valid = vdupq_n_u8(31);

        for block in 0..num_blocks {
            // Safe: offset calculation
            let offset = block * INPUT_BLOCK;

            // Safe: array init and copy
            let mut chars = [0u8; 16];
            chars[..8].copy_from_slice(&encoded_bytes[offset..offset + INPUT_BLOCK]);

            // Unsafe: SIMD load (pointer dereference)
            let char_vec = unsafe { vld1q_u8(chars.as_ptr()) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut idx_vec = vsubq_u8(char_vec, base_offset_vec);

            // Subtract adjustment for each threshold the char exceeds
            for (thresh_vec, adj_vec) in adjusted_thresholds.iter().zip(&adjustment_vecs) {
                // vcgeq returns 0xFF where char >= threshold
                let mask = vcgeq_u8(char_vec, *thresh_vec);
                let adj = vandq_u8(mask, *adj_vec);
                idx_vec = vsubq_u8(idx_vec, adj);
            }

            // SIMD validation
            let invalid_mask = vcgtq_u8(idx_vec, max_valid);
            // Safe: array init
            let mut invalid_bytes = [0u8; 16];
            // Unsafe: SIMD store
            unsafe { vst1q_u8(invalid_bytes.as_mut_ptr(), invalid_mask) };

            // Safe: conditional check
            if invalid_bytes[..8].iter().any(|&b| b != 0) {
                return self.decode_scalar(encoded);
            }

            // Safe: array init
            let mut indices = [0u8; 16];
            // Unsafe: SIMD store
            unsafe { vst1q_u8(indices.as_mut_ptr(), idx_vec) };

            // Safe: bit packing and push
            result.push((indices[0] << 3) | (indices[1] >> 2));
            result.push((indices[1] << 6) | (indices[2] << 1) | (indices[3] >> 4));
            result.push((indices[3] << 4) | (indices[4] >> 1));
            result.push((indices[4] << 7) | (indices[5] << 2) | (indices[6] >> 3));
            result.push((indices[6] << 5) | indices[7]);
        }

        // Safe: remainder handling (no SIMD)
        if simd_chars < encoded_bytes.len() {
            let remainder = &encoded[simd_chars..];
            let bits_per_char = 5usize;
            let mut bit_buffer = 0u32;
            let mut bits_in_buffer = 0usize;

            for ch in remainder.chars() {
                if (ch as u32) >= 256 {
                    return None;
                }
                let index = self.decode_lut[ch as usize];
                if index == 0xFF {
                    return None;
                }
                bit_buffer = (bit_buffer << bits_per_char) | (index as u32);
                bits_in_buffer += bits_per_char;

                while bits_in_buffer >= 8 {
                    bits_in_buffer -= 8;
                    result.push(((bit_buffer >> bits_in_buffer) & 0xFF) as u8);
                }
            }
        }

        Some(result)
    }

    /// NEON 6-bit decoding (base64: 4 chars -> 3 bytes)
    /// Uses SIMD for parallel char-to-index conversion via inverse threshold method
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn decode_neon_6bit(&self, encoded: &str) -> Option<Vec<u8>> {
        use std::arch::aarch64::*;

        // Safe: constants, arithmetic, slice operations
        const INPUT_BLOCK: usize = 4;

        let encoded_bytes = encoded.as_bytes();
        let estimated_len = (encoded_bytes.len() * 3) / 4;
        let mut result = Vec::with_capacity(estimated_len);

        let num_blocks = encoded_bytes.len() / INPUT_BLOCK;
        let simd_chars = num_blocks * INPUT_BLOCK;

        // SIMD vector initialization (safe in target_feature context)
        let base_offset_vec = vdupq_n_u8(self.gap_info.base_offset);

        // Threshold computation logic
        let mut cumulative = 0u8;
        let adjusted_thresholds: Vec<uint8x16_t> = self
            .gap_info
            .thresholds
            .iter()
            .zip(&self.gap_info.adjustments)
            .map(|(&t, &a)| {
                let thresh = self
                    .gap_info
                    .base_offset
                    .wrapping_add(t)
                    .wrapping_add(cumulative);
                cumulative = cumulative.wrapping_add(a);
                vdupq_n_u8(thresh)
            })
            .collect();

        let adjustment_vecs: Vec<uint8x16_t> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| vdupq_n_u8(a))
            .collect();

        let max_valid = vdupq_n_u8(63);

        for block in 0..num_blocks {
            // Safe: offset calculation
            let offset = block * INPUT_BLOCK;

            // Safe: array init and copy
            let mut chars = [0u8; 16];
            chars[..4].copy_from_slice(&encoded_bytes[offset..offset + INPUT_BLOCK]);

            // Unsafe: SIMD load (pointer dereference)
            let char_vec = unsafe { vld1q_u8(chars.as_ptr()) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut idx_vec = vsubq_u8(char_vec, base_offset_vec);

            // Subtract adjustment for each threshold the char exceeds
            for (thresh_vec, adj_vec) in adjusted_thresholds.iter().zip(&adjustment_vecs) {
                let mask = vcgeq_u8(char_vec, *thresh_vec);
                let adj = vandq_u8(mask, *adj_vec);
                idx_vec = vsubq_u8(idx_vec, adj);
            }

            // SIMD validation
            let invalid_mask = vcgtq_u8(idx_vec, max_valid);
            // Safe: array init
            let mut invalid_bytes = [0u8; 16];
            // Unsafe: SIMD store
            unsafe { vst1q_u8(invalid_bytes.as_mut_ptr(), invalid_mask) };

            // Safe: conditional check
            if invalid_bytes[..4].iter().any(|&b| b != 0) {
                return self.decode_scalar(encoded);
            }

            // Safe: array init
            let mut indices = [0u8; 16];
            // Unsafe: SIMD store
            unsafe { vst1q_u8(indices.as_mut_ptr(), idx_vec) };

            // Safe: bit packing and push
            result.push((indices[0] << 2) | (indices[1] >> 4));
            result.push((indices[1] << 4) | (indices[2] >> 2));
            result.push((indices[2] << 6) | indices[3]);
        }

        // Safe: remainder handling (no SIMD)
        if simd_chars < encoded_bytes.len() {
            let remainder = &encoded[simd_chars..];
            let bits_per_char = 6usize;
            let mut bit_buffer = 0u32;
            let mut bits_in_buffer = 0usize;

            for ch in remainder.chars() {
                if (ch as u32) >= 256 {
                    return None;
                }
                let index = self.decode_lut[ch as usize];
                if index == 0xFF {
                    return None;
                }
                bit_buffer = (bit_buffer << bits_per_char) | (index as u32);
                bits_in_buffer += bits_per_char;

                while bits_in_buffer >= 8 {
                    bits_in_buffer -= 8;
                    result.push(((bit_buffer >> bits_in_buffer) & 0xFF) as u8);
                }
            }
        }

        Some(result)
    }

    /// NEON 4-bit decoding (base16: 16 chars -> 8 bytes)
    /// Uses SIMD for parallel char-to-index conversion via inverse threshold method
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn decode_neon_4bit(&self, encoded: &str) -> Option<Vec<u8>> {
        use std::arch::aarch64::*;

        // Safe: constants, arithmetic, slice operations
        const INPUT_BLOCK: usize = 16;

        let encoded_bytes = encoded.as_bytes();
        let estimated_len = encoded_bytes.len() / 2;
        let mut result = Vec::with_capacity(estimated_len);

        let num_blocks = encoded_bytes.len() / INPUT_BLOCK;
        let simd_chars = num_blocks * INPUT_BLOCK;

        // SIMD vector initialization (safe in target_feature context)
        let base_offset_vec = vdupq_n_u8(self.gap_info.base_offset);

        // Threshold computation logic
        let mut cumulative = 0u8;
        let adjusted_thresholds: Vec<uint8x16_t> = self
            .gap_info
            .thresholds
            .iter()
            .zip(&self.gap_info.adjustments)
            .map(|(&t, &a)| {
                let thresh = self
                    .gap_info
                    .base_offset
                    .wrapping_add(t)
                    .wrapping_add(cumulative);
                cumulative = cumulative.wrapping_add(a);
                vdupq_n_u8(thresh)
            })
            .collect();

        let adjustment_vecs: Vec<uint8x16_t> = self
            .gap_info
            .adjustments
            .iter()
            .map(|&a| vdupq_n_u8(a))
            .collect();

        let max_valid = vdupq_n_u8(15);

        for block in 0..num_blocks {
            // Safe: offset calculation
            let offset = block * INPUT_BLOCK;

            // Unsafe: SIMD load (pointer dereference)
            let char_vec = unsafe { vld1q_u8(encoded_bytes[offset..].as_ptr()) };

            // SIMD arithmetic operations (safe in target_feature context)
            let mut idx_vec = vsubq_u8(char_vec, base_offset_vec);

            // Subtract adjustment for each threshold the char exceeds
            for (thresh_vec, adj_vec) in adjusted_thresholds.iter().zip(&adjustment_vecs) {
                let mask = vcgeq_u8(char_vec, *thresh_vec);
                let adj = vandq_u8(mask, *adj_vec);
                idx_vec = vsubq_u8(idx_vec, adj);
            }

            // SIMD validation
            let invalid_mask = vcgtq_u8(idx_vec, max_valid);
            // Safe: array init
            let mut invalid_bytes = [0u8; 16];
            // Unsafe: SIMD store
            unsafe { vst1q_u8(invalid_bytes.as_mut_ptr(), invalid_mask) };

            // Safe: conditional check
            if invalid_bytes.iter().any(|&b| b != 0) {
                return self.decode_scalar(encoded);
            }

            // Safe: array init
            let mut indices = [0u8; 16];
            // Unsafe: SIMD store
            unsafe { vst1q_u8(indices.as_mut_ptr(), idx_vec) };

            // Safe: bit packing and push
            for i in 0..8 {
                result.push((indices[i * 2] << 4) | indices[i * 2 + 1]);
            }
        }

        // Safe: remainder handling (no SIMD)
        if simd_chars < encoded_bytes.len() {
            let remainder = &encoded[simd_chars..];
            let bits_per_char = 4usize;
            let mut bit_buffer = 0u32;
            let mut bits_in_buffer = 0usize;

            for ch in remainder.chars() {
                if (ch as u32) >= 256 {
                    return None;
                }
                let index = self.decode_lut[ch as usize];
                if index == 0xFF {
                    return None;
                }
                bit_buffer = (bit_buffer << bits_per_char) | (index as u32);
                bits_in_buffer += bits_per_char;

                while bits_in_buffer >= 8 {
                    bits_in_buffer -= 8;
                    result.push(((bit_buffer >> bits_in_buffer) & 0xFF) as u8);
                }
            }
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::EncodingMode;

    fn make_geohash_dict() -> Dictionary {
        let chars: Vec<char> = "0123456789bcdefghjkmnpqrstuvwxyz".chars().collect();
        Dictionary::builder()
            .chars(chars)
            .mode(EncodingMode::Chunked)
            .build()
            .unwrap()
    }

    #[test]
    fn test_geohash_gap_detection() {
        let dict = make_geohash_dict();
        let codec = GappedSequentialCodec::from_dictionary(&dict);
        assert!(
            codec.is_some(),
            "Should detect geohash as gapped sequential"
        );

        let codec = codec.unwrap();
        assert_eq!(codec.gap_info.base_offset, 0x30); // '0'
        assert_eq!(codec.gap_info.thresholds.len(), 4); // 4 gaps: a, i, l, o
    }

    #[test]
    fn test_geohash_encode() {
        let dict = make_geohash_dict();
        let codec = GappedSequentialCodec::from_dictionary(&dict).unwrap();

        let encoded = codec.encode(b"Hello", &dict).unwrap();
        assert_eq!(encoded, "91kqsv3g");

        // Verify all chars are valid
        let valid = "0123456789bcdefghjkmnpqrstuvwxyz";
        for c in encoded.chars() {
            assert!(valid.contains(c), "Invalid char: {}", c);
        }
    }

    #[test]
    fn test_geohash_roundtrip() {
        let dict = make_geohash_dict();
        let codec = GappedSequentialCodec::from_dictionary(&dict).unwrap();

        let test_cases = [b"Hello".as_slice(), b"World", b"\x00\xFF", b"test123"];

        for data in test_cases {
            let encoded = codec.encode(data, &dict).unwrap();
            let decoded = codec.decode(&encoded, &dict).unwrap();
            assert_eq!(&decoded[..], data, "Round-trip failed for {:?}", data);
        }
    }

    #[test]
    fn test_crockford_detection() {
        // Crockford: 0-9, A-H, J-K, M-N, P-T, V-Z (missing I, L, O, U)
        let chars: Vec<char> = "0123456789ABCDEFGHJKMNPQRSTVWXYZ".chars().collect();
        let dict = Dictionary::builder()
            .chars(chars)
            .mode(EncodingMode::Chunked)
            .build()
            .unwrap();

        let codec = GappedSequentialCodec::from_dictionary(&dict);
        assert!(
            codec.is_some(),
            "Should detect Crockford as gapped sequential"
        );
    }

    #[test]
    fn test_sequential_rejected() {
        // Pure sequential should be rejected (use GenericSimdCodec instead)
        // Use ASCII range that's actually sequential: 0x40..0x60
        let chars: Vec<char> = (0x40u8..0x60u8).map(|c| c as char).collect();
        let dict = Dictionary::builder()
            .chars(chars)
            .mode(EncodingMode::Chunked)
            .build()
            .unwrap();

        let codec = GappedSequentialCodec::from_dictionary(&dict);
        assert!(
            codec.is_none(),
            "Sequential dict should be rejected by GappedSequentialCodec"
        );
    }
}
