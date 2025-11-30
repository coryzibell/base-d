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
        {
            unsafe {
                self.encode_neon(data, &mut result);
            }
            return Some(result);
        }

        // Scalar fallback
        self.encode_scalar(data, &mut result);
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
        unsafe {
            match self.bits_per_symbol {
                5 => self.encode_ssse3_5bit(data, result),
                6 => self.encode_ssse3_6bit(data, result),
                4 => self.encode_ssse3_4bit(data, result),
                _ => self.encode_scalar(data, result),
            }
        }
    }

    /// SSSE3 5-bit encoding (base32)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3_5bit(&self, data: &[u8], result: &mut String) {
        unsafe {
            use std::arch::x86_64::*;

            const BLOCK_SIZE: usize = 5; // 5 bytes -> 8 chars

            if data.len() < BLOCK_SIZE {
                self.encode_scalar(data, result);
                return;
            }

            // Precompute threshold vectors
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

            let num_blocks = data.len() / BLOCK_SIZE;
            let simd_bytes = num_blocks * BLOCK_SIZE;

            let mut offset = 0;
            for _ in 0..num_blocks {
                // Load 5 bytes and extract 8 x 5-bit indices
                debug_assert!(
                    offset + 5 <= data.len(),
                    "SIMD bounds check: offset {} + 5 exceeds len {}",
                    offset,
                    data.len()
                );
                let b0 = *data.get_unchecked(offset);
                let b1 = *data.get_unchecked(offset + 1);
                let b2 = *data.get_unchecked(offset + 2);
                let b3 = *data.get_unchecked(offset + 3);
                let b4 = *data.get_unchecked(offset + 4);

                let mut indices = [0u8; 16];
                indices[0] = (b0 >> 3) & 0x1F;
                indices[1] = ((b0 << 2) | (b1 >> 6)) & 0x1F;
                indices[2] = (b1 >> 1) & 0x1F;
                indices[3] = ((b1 << 4) | (b2 >> 4)) & 0x1F;
                indices[4] = ((b2 << 1) | (b3 >> 7)) & 0x1F;
                indices[5] = (b3 >> 2) & 0x1F;
                indices[6] = ((b3 << 3) | (b4 >> 5)) & 0x1F;
                indices[7] = b4 & 0x1F;

                // Load indices into SIMD register
                let idx_vec = _mm_loadu_si128(indices.as_ptr() as *const __m128i);

                // Start with base offset + index
                let mut char_vec = _mm_add_epi8(base_offset_vec, idx_vec);

                // Add adjustment for each threshold
                for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                    // cmpgt returns 0xFF where idx > threshold-1, i.e., idx >= threshold
                    let mask = _mm_cmpgt_epi8(idx_vec, *thresh_vec);
                    // AND with adjustment to get conditional add
                    let adj = _mm_and_si128(mask, *adj_vec);
                    char_vec = _mm_add_epi8(char_vec, adj);
                }

                // Store result (only first 8 bytes are valid)
                let mut output = [0u8; 16];
                _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, char_vec);

                // Push to result
                for &ch in &output[..8] {
                    result.push(ch as char);
                }

                offset += BLOCK_SIZE;
            }

            // Handle remainder
            if simd_bytes < data.len() {
                self.encode_scalar(&data[simd_bytes..], result);
            }
        }
    }

    /// SSSE3 6-bit encoding (base64)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3_6bit(&self, data: &[u8], result: &mut String) {
        unsafe {
            use std::arch::x86_64::*;

            const BLOCK_SIZE: usize = 3; // 3 bytes -> 4 chars

            if data.len() < BLOCK_SIZE {
                self.encode_scalar(data, result);
                return;
            }

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

            let num_blocks = data.len() / BLOCK_SIZE;
            let simd_bytes = num_blocks * BLOCK_SIZE;

            let mut offset = 0;
            for _ in 0..num_blocks {
                debug_assert!(
                    offset + 3 <= data.len(),
                    "SIMD bounds check: offset {} + 3 exceeds len {}",
                    offset,
                    data.len()
                );
                let b0 = *data.get_unchecked(offset);
                let b1 = *data.get_unchecked(offset + 1);
                let b2 = *data.get_unchecked(offset + 2);

                let mut indices = [0u8; 16];
                indices[0] = (b0 >> 2) & 0x3F;
                indices[1] = ((b0 << 4) | (b1 >> 4)) & 0x3F;
                indices[2] = ((b1 << 2) | (b2 >> 6)) & 0x3F;
                indices[3] = b2 & 0x3F;

                let idx_vec = _mm_loadu_si128(indices.as_ptr() as *const __m128i);
                let mut char_vec = _mm_add_epi8(base_offset_vec, idx_vec);

                for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                    let mask = _mm_cmpgt_epi8(idx_vec, *thresh_vec);
                    let adj = _mm_and_si128(mask, *adj_vec);
                    char_vec = _mm_add_epi8(char_vec, adj);
                }

                let mut output = [0u8; 16];
                _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, char_vec);

                for &ch in &output[..4] {
                    result.push(ch as char);
                }

                offset += BLOCK_SIZE;
            }

            if simd_bytes < data.len() {
                self.encode_scalar(&data[simd_bytes..], result);
            }
        }
    }

    /// SSSE3 4-bit encoding (base16)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3_4bit(&self, data: &[u8], result: &mut String) {
        unsafe {
            use std::arch::x86_64::*;

            const BLOCK_SIZE: usize = 8; // 8 bytes -> 16 chars

            if data.len() < BLOCK_SIZE {
                self.encode_scalar(data, result);
                return;
            }

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

            let num_blocks = data.len() / BLOCK_SIZE;
            let simd_bytes = num_blocks * BLOCK_SIZE;

            let mut offset = 0;
            for _ in 0..num_blocks {
                // Extract 16 x 4-bit indices from 8 bytes
                debug_assert!(
                    offset + 8 <= data.len(),
                    "SIMD bounds check: offset {} + 8 exceeds len {}",
                    offset,
                    data.len()
                );
                let mut indices = [0u8; 16];
                for i in 0..8 {
                    let byte = *data.get_unchecked(offset + i);
                    indices[i * 2] = (byte >> 4) & 0x0F;
                    indices[i * 2 + 1] = byte & 0x0F;
                }

                let idx_vec = _mm_loadu_si128(indices.as_ptr() as *const __m128i);
                let mut char_vec = _mm_add_epi8(base_offset_vec, idx_vec);

                for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                    let mask = _mm_cmpgt_epi8(idx_vec, *thresh_vec);
                    let adj = _mm_and_si128(mask, *adj_vec);
                    char_vec = _mm_add_epi8(char_vec, adj);
                }

                let mut output = [0u8; 16];
                _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, char_vec);

                for &ch in &output[..16] {
                    result.push(ch as char);
                }

                offset += BLOCK_SIZE;
            }

            if simd_bytes < data.len() {
                self.encode_scalar(&data[simd_bytes..], result);
            }
        }
    }

    /// NEON encoding implementation
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon(&self, data: &[u8], result: &mut String) {
        unsafe {
            match self.bits_per_symbol {
                5 => self.encode_neon_5bit(data, result),
                6 => self.encode_neon_6bit(data, result),
                4 => self.encode_neon_4bit(data, result),
                _ => self.encode_scalar(data, result),
            }
        }
    }

    /// NEON 5-bit encoding (base32)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_5bit(&self, data: &[u8], result: &mut String) {
        unsafe {
            use std::arch::aarch64::*;

            const BLOCK_SIZE: usize = 5;

            if data.len() < BLOCK_SIZE {
                self.encode_scalar(data, result);
                return;
            }

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

            let num_blocks = data.len() / BLOCK_SIZE;
            let simd_bytes = num_blocks * BLOCK_SIZE;

            let mut offset = 0;
            for _ in 0..num_blocks {
                debug_assert!(
                    offset + 5 <= data.len(),
                    "SIMD bounds check: offset {} + 5 exceeds len {}",
                    offset,
                    data.len()
                );
                let b0 = *data.get_unchecked(offset);
                let b1 = *data.get_unchecked(offset + 1);
                let b2 = *data.get_unchecked(offset + 2);
                let b3 = *data.get_unchecked(offset + 3);
                let b4 = *data.get_unchecked(offset + 4);

                let mut indices = [0u8; 16];
                indices[0] = (b0 >> 3) & 0x1F;
                indices[1] = ((b0 << 2) | (b1 >> 6)) & 0x1F;
                indices[2] = (b1 >> 1) & 0x1F;
                indices[3] = ((b1 << 4) | (b2 >> 4)) & 0x1F;
                indices[4] = ((b2 << 1) | (b3 >> 7)) & 0x1F;
                indices[5] = (b3 >> 2) & 0x1F;
                indices[6] = ((b3 << 3) | (b4 >> 5)) & 0x1F;
                indices[7] = b4 & 0x1F;

                let idx_vec = vld1q_u8(indices.as_ptr());
                let mut char_vec = vaddq_u8(base_offset_vec, idx_vec);

                for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                    // vcgeq returns 0xFF where idx >= threshold
                    let mask = vcgeq_u8(idx_vec, *thresh_vec);
                    let adj = vandq_u8(mask, *adj_vec);
                    char_vec = vaddq_u8(char_vec, adj);
                }

                let mut output = [0u8; 16];
                vst1q_u8(output.as_mut_ptr(), char_vec);

                for &ch in &output[..8] {
                    result.push(ch as char);
                }

                offset += BLOCK_SIZE;
            }

            if simd_bytes < data.len() {
                self.encode_scalar(&data[simd_bytes..], result);
            }
        }
    }

    /// NEON 6-bit encoding (base64)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_6bit(&self, data: &[u8], result: &mut String) {
        unsafe {
            use std::arch::aarch64::*;

            const BLOCK_SIZE: usize = 3;

            if data.len() < BLOCK_SIZE {
                self.encode_scalar(data, result);
                return;
            }

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

            let num_blocks = data.len() / BLOCK_SIZE;
            let simd_bytes = num_blocks * BLOCK_SIZE;

            let mut offset = 0;
            for _ in 0..num_blocks {
                debug_assert!(
                    offset + 3 <= data.len(),
                    "SIMD bounds check: offset {} + 3 exceeds len {}",
                    offset,
                    data.len()
                );
                let b0 = *data.get_unchecked(offset);
                let b1 = *data.get_unchecked(offset + 1);
                let b2 = *data.get_unchecked(offset + 2);

                let mut indices = [0u8; 16];
                indices[0] = (b0 >> 2) & 0x3F;
                indices[1] = ((b0 << 4) | (b1 >> 4)) & 0x3F;
                indices[2] = ((b1 << 2) | (b2 >> 6)) & 0x3F;
                indices[3] = b2 & 0x3F;

                let idx_vec = vld1q_u8(indices.as_ptr());
                let mut char_vec = vaddq_u8(base_offset_vec, idx_vec);

                for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                    let mask = vcgeq_u8(idx_vec, *thresh_vec);
                    let adj = vandq_u8(mask, *adj_vec);
                    char_vec = vaddq_u8(char_vec, adj);
                }

                let mut output = [0u8; 16];
                vst1q_u8(output.as_mut_ptr(), char_vec);

                for &ch in &output[..4] {
                    result.push(ch as char);
                }

                offset += BLOCK_SIZE;
            }

            if simd_bytes < data.len() {
                self.encode_scalar(&data[simd_bytes..], result);
            }
        }
    }

    /// NEON 4-bit encoding (base16)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_4bit(&self, data: &[u8], result: &mut String) {
        unsafe {
            use std::arch::aarch64::*;

            const BLOCK_SIZE: usize = 8;

            if data.len() < BLOCK_SIZE {
                self.encode_scalar(data, result);
                return;
            }

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

            let num_blocks = data.len() / BLOCK_SIZE;
            let simd_bytes = num_blocks * BLOCK_SIZE;

            let mut offset = 0;
            for _ in 0..num_blocks {
                debug_assert!(
                    offset + 8 <= data.len(),
                    "SIMD bounds check: offset {} + 8 exceeds len {}",
                    offset,
                    data.len()
                );
                let mut indices = [0u8; 16];
                for i in 0..8 {
                    let byte = *data.get_unchecked(offset + i);
                    indices[i * 2] = (byte >> 4) & 0x0F;
                    indices[i * 2 + 1] = byte & 0x0F;
                }

                let idx_vec = vld1q_u8(indices.as_ptr());
                let mut char_vec = vaddq_u8(base_offset_vec, idx_vec);

                for (thresh_vec, adj_vec) in threshold_vecs.iter().zip(&adjustment_vecs) {
                    let mask = vcgeq_u8(idx_vec, *thresh_vec);
                    let adj = vandq_u8(mask, *adj_vec);
                    char_vec = vaddq_u8(char_vec, adj);
                }

                let mut output = [0u8; 16];
                vst1q_u8(output.as_mut_ptr(), char_vec);

                for &ch in &output[..16] {
                    result.push(ch as char);
                }

                offset += BLOCK_SIZE;
            }

            if simd_bytes < data.len() {
                self.encode_scalar(&data[simd_bytes..], result);
            }
        }
    }

    /// Decode encoded string back to bytes
    pub fn decode(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        if encoded.is_empty() {
            return Some(Vec::new());
        }

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
