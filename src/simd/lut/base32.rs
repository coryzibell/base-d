//! Base32-specific SIMD implementations for Base64LutCodec
//!
//! This module contains all base32 (5-bit encoding) SIMD implementations,
//! separated from base64 (6-bit) code for clarity and maintainability.
//!
//! Key characteristics of base32:
//! - 5-bit indices (32 possible values)
//! - 5 bytes encode to 8 characters (40 bits)
//! - 8 characters decode to 5 bytes
//! - 16 characters decode to 10 bytes (SIMD block size)

use super::base64::Base64LutCodec;

impl Base64LutCodec {
    // ========================================================================
    // NEON (aarch64) ENCODING
    // ========================================================================

    /// NEON base32 encode (5-bit indices)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    pub(super) unsafe fn encode_neon_base32(&self, data: &[u8], result: &mut String) {
        use std::arch::aarch64::*;

        const BLOCK_SIZE: usize = 5; // 5 bytes -> 8 chars (40 bits)

        if data.len() < BLOCK_SIZE {
            self.encode_scalar_base32(data, result);
            return;
        }

        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Unsafe: NEON intrinsics for loading LUT
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
            // SAFETY: offset + BLOCK_SIZE <= simd_bytes <= data.len() by construction
            // (num_blocks = data.len() / BLOCK_SIZE, simd_bytes = num_blocks * BLOCK_SIZE)
            debug_assert!(offset + BLOCK_SIZE <= data.len());

            // Unsafe: unchecked indexing
            let bytes = unsafe {
                [
                    *data.get_unchecked(offset),
                    *data.get_unchecked(offset + 1),
                    *data.get_unchecked(offset + 2),
                    *data.get_unchecked(offset + 3),
                    *data.get_unchecked(offset + 4),
                ]
            };

            // Safe: bit manipulation
            let mut indices = [0u8; 16];
            indices[0] = (bytes[0] >> 3) & 0x1F; // bits 7-3
            indices[1] = ((bytes[0] << 2) | (bytes[1] >> 6)) & 0x1F; // bits 2-0, 7-6
            indices[2] = (bytes[1] >> 1) & 0x1F; // bits 5-1
            indices[3] = ((bytes[1] << 4) | (bytes[2] >> 4)) & 0x1F; // bits 0, 7-4
            indices[4] = ((bytes[2] << 1) | (bytes[3] >> 7)) & 0x1F; // bits 3-0, 7
            indices[5] = (bytes[3] >> 2) & 0x1F; // bits 6-2
            indices[6] = ((bytes[3] << 3) | (bytes[4] >> 5)) & 0x1F; // bits 1-0, 7-5
            indices[7] = bytes[4] & 0x1F; // bits 4-0

            // Unsafe: NEON intrinsics
            let chars = unsafe {
                let idx_vec = vld1q_u8(indices.as_ptr());
                vqtbl4q_u8(lut_tables, idx_vec)
            };

            // Unsafe: NEON store
            let mut output_buf = [0u8; 16];
            unsafe {
                vst1q_u8(output_buf.as_mut_ptr(), chars);
            }

            // Safe: iteration, push
            for &byte in &output_buf[0..8] {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Safe: scalar fallback
        if simd_bytes < data.len() {
            self.encode_scalar_base32(&data[simd_bytes..], result);
        }
    }

    /// Scalar fallback for base32 encoding
    #[cfg(target_arch = "aarch64")]
    pub(super) fn encode_scalar_base32(&self, data: &[u8], result: &mut String) {
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

    // ========================================================================
    // x86_64 ENCODING
    // ========================================================================

    /// SSE range-reduction base32 encode (5-bit indices)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    pub(super) unsafe fn encode_ssse3_range_reduction_5bit(
        &self,
        data: &[u8],
        result: &mut String,
    ) {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 5; // 5 bytes → 8 chars (40 bits)

        if data.len() < BLOCK_SIZE {
            self.encode_scalar_base32_x86(data, result);
            return;
        }

        let range_info = self.range_info.as_ref().unwrap();
        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Unsafe: SIMD intrinsics for loading LUT
        let (offset_lut, subs_threshold) = unsafe {
            (
                _mm_loadu_si128(range_info.offset_lut.as_ptr() as *const __m128i),
                _mm_set1_epi8(range_info.subs_threshold as i8),
            )
        };

        let mut offset = 0;
        for _ in 0..num_blocks {
            // SAFETY: offset + BLOCK_SIZE <= simd_bytes <= data.len() by construction
            // (num_blocks = data.len() / BLOCK_SIZE, simd_bytes = num_blocks * BLOCK_SIZE)
            debug_assert!(offset + BLOCK_SIZE <= data.len());

            // Unsafe: unchecked indexing
            let bytes = unsafe {
                [
                    *data.get_unchecked(offset),
                    *data.get_unchecked(offset + 1),
                    *data.get_unchecked(offset + 2),
                    *data.get_unchecked(offset + 3),
                    *data.get_unchecked(offset + 4),
                ]
            };

            // Safe: bit manipulation
            let mut indices = [0u8; 16];
            indices[0] = (bytes[0] >> 3) & 0x1F;
            indices[1] = ((bytes[0] << 2) | (bytes[1] >> 6)) & 0x1F;
            indices[2] = (bytes[1] >> 1) & 0x1F;
            indices[3] = ((bytes[1] << 4) | (bytes[2] >> 4)) & 0x1F;
            indices[4] = ((bytes[2] << 1) | (bytes[3] >> 7)) & 0x1F;
            indices[5] = (bytes[3] >> 2) & 0x1F;
            indices[6] = ((bytes[3] << 3) | (bytes[4] >> 5)) & 0x1F;
            indices[7] = bytes[4] & 0x1F;

            // Unsafe: SIMD intrinsics
            let chars = unsafe {
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
                _mm_add_epi8(idx_vec, offset_vec)
            };

            // Unsafe: SIMD store
            let mut output_buf = [0u8; 16];
            unsafe {
                _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, chars);
            }

            // Safe: iteration, push
            for &byte in &output_buf[0..8] {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Safe: scalar remainder
        if simd_bytes < data.len() {
            self.encode_scalar_base32_x86(&data[simd_bytes..], result);
        }
    }

    /// AVX-512 VBMI base32 encode (5-bit indices)
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512vbmi"))]
    #[target_feature(enable = "avx512vbmi")]
    pub(super) unsafe fn encode_avx512_vbmi_base32(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 5; // 5 bytes -> 8 chars (40 bits)

        if data.len() < BLOCK_SIZE {
            self.encode_scalar_base32_x86(data, result);
            return;
        }

        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Unsafe: AVX-512 intrinsic for loading LUT
        let lut = unsafe { _mm512_loadu_si512(self.encode_lut.as_ptr() as *const i32) };

        let mut offset = 0;
        for _ in 0..num_blocks {
            // SAFETY: offset + BLOCK_SIZE <= simd_bytes <= data.len() by construction
            // (num_blocks = data.len() / BLOCK_SIZE, simd_bytes = num_blocks * BLOCK_SIZE)
            debug_assert!(offset + BLOCK_SIZE <= data.len());

            // Unsafe: unchecked indexing
            let bytes = unsafe {
                [
                    *data.get_unchecked(offset),
                    *data.get_unchecked(offset + 1),
                    *data.get_unchecked(offset + 2),
                    *data.get_unchecked(offset + 3),
                    *data.get_unchecked(offset + 4),
                ]
            };

            // Safe: bit manipulation
            let mut indices = [0u8; 64]; // ZMM is 64 bytes, but we only use first 8
            indices[0] = (bytes[0] >> 3) & 0x1F;
            indices[1] = ((bytes[0] << 2) | (bytes[1] >> 6)) & 0x1F;
            indices[2] = (bytes[1] >> 1) & 0x1F;
            indices[3] = ((bytes[1] << 4) | (bytes[2] >> 4)) & 0x1F;
            indices[4] = ((bytes[2] << 1) | (bytes[3] >> 7)) & 0x1F;
            indices[5] = (bytes[3] >> 2) & 0x1F;
            indices[6] = ((bytes[3] << 3) | (bytes[4] >> 5)) & 0x1F;
            indices[7] = bytes[4] & 0x1F;

            // Unsafe: AVX-512 intrinsics
            let chars = unsafe {
                let idx_vec = _mm512_loadu_si512(indices.as_ptr() as *const i32);
                _mm512_permutexvar_epi8(idx_vec, lut)
            };

            // Unsafe: AVX-512 store
            let mut output_buf = [0u8; 64];
            unsafe {
                _mm512_storeu_si512(output_buf.as_mut_ptr() as *mut i32, chars);
            }

            // Safe: iteration, push
            for &byte in &output_buf[0..8] {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Safe: scalar remainder
        if simd_bytes < data.len() {
            self.encode_scalar_base32_x86(&data[simd_bytes..], result);
        }
    }

    /// Scalar fallback for base32 encoding (x86)
    #[cfg(target_arch = "x86_64")]
    pub(super) fn encode_scalar_base32_x86(&self, data: &[u8], result: &mut String) {
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

    // ========================================================================
    // HELPER FUNCTIONS
    // ========================================================================

    /// Check if dictionary is RFC4648 base32
    pub(super) fn is_rfc4648_base32(&self) -> bool {
        if self.metadata.base != 32 {
            return false;
        }
        // RFC4648: "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
        let expected = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        &self.encode_lut[..32] == expected
    }

    // ========================================================================
    // x86_64 DECODING
    // ========================================================================

    /// Multi-range decode for base32 (5-bit indices)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    pub(super) unsafe fn decode_ssse3_multi_range_5bit(
        &self,
        encoded: &[u8],
        result: &mut Vec<u8>,
    ) -> bool {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 16; // 16 chars → 10 bytes

        let range_info = self.range_info.as_ref().unwrap();
        let num_blocks = encoded.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for i in 0..num_blocks {
            let offset = i * BLOCK_SIZE;

            // SAFETY: offset + BLOCK_SIZE <= simd_bytes <= encoded.len() by construction
            // (num_blocks = encoded.len() / BLOCK_SIZE, offset = i * BLOCK_SIZE where i < num_blocks)
            debug_assert!(offset + BLOCK_SIZE <= encoded.len());

            // Unsafe: SIMD load
            let chars = unsafe { _mm_loadu_si128(encoded.as_ptr().add(offset) as *const __m128i) };

            // Safe: validation + translation (uses SIMD internally)
            let indices =
                match unsafe { self.validate_and_translate_multi_range(chars, range_info) } {
                    Some(idx) => idx,
                    None => return false,
                };

            // Safe: unpacking (uses SIMD internally)
            let bytes = unsafe { self.unpack_5bit_ssse3(indices) };

            // Safe: extend from slice
            result.extend_from_slice(&bytes);
        }

        // Safe: scalar remainder
        if simd_bytes < encoded.len() && !self.decode_scalar(&encoded[simd_bytes..], result) {
            return false;
        }

        true
    }

    /// Unpack 16×5-bit indices to 10×8-bit bytes (SIMD-accelerated)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    pub(super) unsafe fn unpack_5bit_ssse3(&self, indices: std::arch::x86_64::__m128i) -> [u8; 10] {
        use std::arch::x86_64::*;

        // Unsafe: SIMD store
        let mut idx_buf = [0u8; 16];
        unsafe {
            _mm_storeu_si128(idx_buf.as_mut_ptr() as *mut __m128i, indices);
        }

        // Safe: bit manipulation
        idx_buf.iter_mut().for_each(|val| *val &= 0x1F);

        // Safe: bit packing
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
    pub(super) unsafe fn decode_ssse3_base32_rfc4648(
        &self,
        encoded: &[u8],
        result: &mut Vec<u8>,
    ) -> bool {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 16; // 16 chars per iteration

        let num_blocks = encoded.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for i in 0..num_blocks {
            let offset = i * BLOCK_SIZE;

            // SAFETY: offset + BLOCK_SIZE <= simd_bytes <= encoded.len() by construction
            // (num_blocks = encoded.len() / BLOCK_SIZE, offset = i * BLOCK_SIZE where i < num_blocks)
            debug_assert!(offset + BLOCK_SIZE <= encoded.len());

            // Unsafe: SIMD load and validation
            let indices = unsafe {
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
                _mm_blendv_epi8(digit_indices, letter_indices, in_range1)
            };

            // Unsafe: SIMD store
            let mut idx_buf = [0u8; 16];
            unsafe {
                _mm_storeu_si128(idx_buf.as_mut_ptr() as *mut __m128i, indices);
            }

            // Safe: bit packing
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

        // Safe: scalar remainder
        if simd_bytes < encoded.len() && !self.decode_scalar(&encoded[simd_bytes..], result) {
            return false;
        }

        true
    }

    // ========================================================================
    // NEON (aarch64) DECODING
    // ========================================================================

    /// NEON base32 RFC4648 decode (range-based validation)
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    pub(super) unsafe fn decode_neon_base32_rfc4648(
        &self,
        encoded: &[u8],
        result: &mut Vec<u8>,
    ) -> bool {
        use std::arch::aarch64::*;

        const BLOCK_SIZE: usize = 16;

        let num_blocks = encoded.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        for i in 0..num_blocks {
            let offset = i * BLOCK_SIZE;

            // SAFETY: offset + BLOCK_SIZE <= simd_bytes <= encoded.len() by construction
            // (num_blocks = encoded.len() / BLOCK_SIZE, offset = i * BLOCK_SIZE where i < num_blocks)
            debug_assert!(offset + BLOCK_SIZE <= encoded.len());

            // Unsafe: NEON load and validation
            let indices = unsafe {
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
                vbslq_u8(in_range1, letter_indices, digit_indices)
            };

            // Unsafe: NEON store
            let mut idx_buf = [0u8; 16];
            unsafe {
                vst1q_u8(idx_buf.as_mut_ptr(), indices);
            }

            // Safe: bit packing
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

        // Safe: scalar remainder
        if simd_bytes < encoded.len() {
            if !self.decode_scalar(&encoded[simd_bytes..], result) {
                return false;
            }
        }

        true
    }
}
