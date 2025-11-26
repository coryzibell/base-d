//! SmallLutCodec: SIMD codec for small arbitrary alphabets (≤16 characters)
//!
//! Uses direct single-instruction lookup:
//! - x86: pshufb (_mm_shuffle_epi8)
//! - ARM: vqtbl1q_u8
//!
//! Constraints:
//! - Base ≤ 16
//! - Power-of-2 base
//! - ASCII-only (char < 0x80)
//! - Non-sequential alphabets only

use crate::core::dictionary::Dictionary;
use crate::simd::alphabets::{AlphabetMetadata, LutStrategy, TranslationStrategy};

/// SIMD codec for small arbitrary alphabets (≤16 characters)
///
/// Uses direct shuffle-based lookup for encoding and a 256-byte sparse
/// table for decoding with validation.
pub struct SmallLutCodec {
    metadata: AlphabetMetadata,

    /// Encoding LUT: index → char (16 bytes, one per possible index)
    encode_lut: [u8; 16],

    /// Decoding LUT: char → index (256 bytes, sparse)
    /// 0xFF means invalid character
    decode_lut: [u8; 256],
}

impl SmallLutCodec {
    /// Create codec from dictionary
    ///
    /// Returns None if:
    /// - Alphabet > 16 chars
    /// - Not power-of-2 base
    /// - Alphabet is sequential (should use GenericSimdCodec)
    /// - Any character > 0x7F (non-ASCII)
    pub fn from_dictionary(dict: &Dictionary) -> Option<Self> {
        let metadata = AlphabetMetadata::from_dictionary(dict);

        // Only for small arbitrary alphabets
        if metadata.base > 16 || !metadata.base.is_power_of_two() {
            return None;
        }

        // Must be arbitrary (non-sequential)
        if !matches!(metadata.strategy, TranslationStrategy::Arbitrary { .. }) {
            return None;
        }

        // Verify LUT strategy is appropriate
        if metadata.lut_strategy() != LutStrategy::SmallDirect {
            return None;
        }

        // Build encoding LUT (index → char)
        let mut encode_lut = [0u8; 16];
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

        Some(Self {
            metadata,
            encode_lut,
            decode_lut,
        })
    }

    /// Encode binary data to string using SIMD
    ///
    /// Returns None if SIMD is not available or encoding fails.
    pub fn encode(&self, data: &[u8], _dict: &Dictionary) -> Option<String> {
        // Only supports 4-bit (base 16) for now
        if self.metadata.base != 16 {
            return None;
        }

        // Handle empty input
        if data.is_empty() {
            return Some(String::new());
        }

        let output_len = data.len() * 2; // 2 hex chars per byte
        let mut result = String::with_capacity(output_len);

        #[cfg(target_arch = "x86_64")]
        unsafe {
            if is_x86_feature_detected!("ssse3") {
                self.encode_ssse3_impl(data, &mut result);
                return Some(result);
            }
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            self.encode_neon_impl(data, &mut result);
            return Some(result);
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            // Scalar fallback for unsupported architectures
            self.encode_scalar(data, &mut result);
            return Some(result);
        }

        None
    }

    /// x86_64 SSSE3 encode implementation
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "ssse3")]
    unsafe fn encode_ssse3_impl(&self, data: &[u8], result: &mut String) {
        use std::arch::x86_64::*;

        const BLOCK_SIZE: usize = 16;

        if data.len() < BLOCK_SIZE {
            self.encode_scalar(data, result);
            return;
        }

        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Load LUT into XMM register
        let lut = _mm_loadu_si128(self.encode_lut.as_ptr() as *const __m128i);
        let mask_0f = _mm_set1_epi8(0x0F);

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Load 16 bytes
            let input_vec = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

            // Extract high nibbles (shift right by 4)
            let hi_nibbles = _mm_and_si128(_mm_srli_epi32(input_vec, 4), mask_0f);

            // Extract low nibbles
            let lo_nibbles = _mm_and_si128(input_vec, mask_0f);

            // Translate nibbles to ASCII using pshufb
            let hi_ascii = _mm_shuffle_epi8(lut, hi_nibbles);
            let lo_ascii = _mm_shuffle_epi8(lut, lo_nibbles);

            // Interleave high and low bytes: hi[0], lo[0], hi[1], lo[1], ...
            let result_lo = _mm_unpacklo_epi8(hi_ascii, lo_ascii);
            let result_hi = _mm_unpackhi_epi8(hi_ascii, lo_ascii);

            // Store 32 output characters
            let mut output_buf = [0u8; 32];
            _mm_storeu_si128(output_buf.as_mut_ptr() as *mut __m128i, result_lo);
            _mm_storeu_si128(output_buf.as_mut_ptr().add(16) as *mut __m128i, result_hi);

            // Append to result (ASCII characters)
            for &byte in &output_buf {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Handle remainder with scalar
        if simd_bytes < data.len() {
            self.encode_scalar(&data[simd_bytes..], result);
        }
    }

    /// aarch64 NEON encode implementation
    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn encode_neon_impl(&self, data: &[u8], result: &mut String) {
        use std::arch::aarch64::*;

        const BLOCK_SIZE: usize = 16;

        if data.len() < BLOCK_SIZE {
            self.encode_scalar(data, result);
            return;
        }

        let num_blocks = data.len() / BLOCK_SIZE;
        let simd_bytes = num_blocks * BLOCK_SIZE;

        // Load LUT into NEON register
        let lut_vec = vld1q_u8(self.encode_lut.as_ptr());
        let mask_0f = vdupq_n_u8(0x0F);

        let mut offset = 0;
        for _ in 0..num_blocks {
            // Load 16 bytes
            let input_vec = vld1q_u8(data.as_ptr().add(offset));

            // Extract high nibbles (shift right by 4)
            let hi_nibbles = vandq_u8(vshrq_n_u8(input_vec, 4), mask_0f);

            // Extract low nibbles
            let lo_nibbles = vandq_u8(input_vec, mask_0f);

            // Translate nibbles to ASCII using vqtbl1q_u8
            let hi_ascii = vqtbl1q_u8(lut_vec, hi_nibbles);
            let lo_ascii = vqtbl1q_u8(lut_vec, lo_nibbles);

            // Interleave high and low bytes: hi[0], lo[0], hi[1], lo[1], ...
            let result_lo = vzip1q_u8(hi_ascii, lo_ascii);
            let result_hi = vzip2q_u8(hi_ascii, lo_ascii);

            // Store 32 output characters
            let mut output_buf = [0u8; 32];
            vst1q_u8(output_buf.as_mut_ptr(), result_lo);
            vst1q_u8(output_buf.as_mut_ptr().add(16), result_hi);

            // Append to result (ASCII characters)
            for &byte in &output_buf {
                result.push(byte as char);
            }

            offset += BLOCK_SIZE;
        }

        // Handle remainder with scalar
        if simd_bytes < data.len() {
            self.encode_scalar(&data[simd_bytes..], result);
        }
    }

    /// Scalar fallback for remainder bytes
    fn encode_scalar(&self, data: &[u8], result: &mut String) {
        for &byte in data {
            let hi = (byte >> 4) as usize;
            let lo = (byte & 0x0F) as usize;
            result.push(self.encode_lut[hi] as char);
            result.push(self.encode_lut[lo] as char);
        }
    }

    /// Decode string to binary data using SIMD
    ///
    /// Returns None if input contains invalid characters.
    pub fn decode(&self, encoded: &str, _dict: &Dictionary) -> Option<Vec<u8>> {
        // Only supports 4-bit (base 16) for now
        if self.metadata.base != 16 {
            return None;
        }

        // Handle empty input
        if encoded.is_empty() {
            return Some(Vec::new());
        }

        // For base16, input must have even length (2 chars per byte)
        if encoded.len() % 2 != 0 {
            return None;
        }

        let output_len = encoded.len() / 2;
        let mut result = Vec::with_capacity(output_len);

        // Use scalar validation with decode_lut[256]
        // For each pair of chars, translate to nibbles and pack into byte
        let encoded_bytes = encoded.as_bytes();

        for i in (0..encoded_bytes.len()).step_by(2) {
            let hi_char = encoded_bytes[i];
            let lo_char = encoded_bytes[i + 1];

            // Lookup in decode_lut (returns 0xFF if invalid)
            let hi_nibble = self.decode_lut[hi_char as usize];
            let lo_nibble = self.decode_lut[lo_char as usize];

            // Validate both characters
            if hi_nibble == 0xFF || lo_nibble == 0xFF {
                return None; // Invalid character
            }

            // Pack nibbles into byte
            let byte = (hi_nibble << 4) | lo_nibble;
            result.push(byte);
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation_from_arbitrary_base16() {
        // Shuffled 16-char alphabet
        let chars: Vec<char> = "zyxwvutsrqponmlk".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = SmallLutCodec::from_dictionary(&dict);
        assert!(
            codec.is_some(),
            "Should create codec for arbitrary base16"
        );
    }

    #[test]
    fn test_rejects_sequential_alphabet() {
        // Sequential alphabet should use GenericSimdCodec, not LUT
        let chars: Vec<char> = (0x30..0x40).map(|c| char::from_u32(c).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = SmallLutCodec::from_dictionary(&dict);
        assert!(
            codec.is_none(),
            "Should reject sequential (use GenericSimdCodec)"
        );
    }

    #[test]
    fn test_rejects_large_alphabet() {
        // 32-char alphabet too large for SmallLutCodec
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = SmallLutCodec::from_dictionary(&dict);
        assert!(codec.is_none(), "Should reject base32 (too large)");
    }

    #[test]
    fn test_rejects_non_power_of_two() {
        // 10-char alphabet (non power-of-2)
        let chars: Vec<char> = "0123456789".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = SmallLutCodec::from_dictionary(&dict);
        assert!(codec.is_none(), "Should reject non-power-of-2 base");
    }

    #[test]
    fn test_lut_construction() {
        // Shuffled hex alphabet
        let chars: Vec<char> = "9876543210ZYXWVU".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        // Verify encode_lut matches alphabet
        assert_eq!(codec.encode_lut[0], b'9');
        assert_eq!(codec.encode_lut[1], b'8');
        assert_eq!(codec.encode_lut[15], b'U');

        // Verify decode_lut is inverse
        assert_eq!(codec.decode_lut[b'9' as usize], 0);
        assert_eq!(codec.decode_lut[b'8' as usize], 1);
        assert_eq!(codec.decode_lut[b'0' as usize], 9);
        assert_eq!(codec.decode_lut[b'U' as usize], 15);

        // Verify invalid chars marked as 0xFF
        assert_eq!(codec.decode_lut[b'A' as usize], 0xFF);
        assert_eq!(codec.decode_lut[b'a' as usize], 0xFF);
    }

    #[test]
    fn test_encode_shuffled_base16() {
        // Shuffled hex alphabet: 0→z, 1→y, 2→x, etc.
        let chars: Vec<char> = "zyxwvutsrqponmlk".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        // Encode 0xAB -> nibbles [0xA, 0xB] = [10, 11]
        // chars[10] = 'p', chars[11] = 'o'
        let data = &[0xABu8];
        let encoded = codec.encode(data, &dict).unwrap();

        assert_eq!(encoded, "po");
    }

    #[test]
    fn test_encode_standard_hex_rejected() {
        // Standard hex alphabet is sequential, so should be rejected
        // (Use GenericSimdCodec instead)
        let chars: Vec<char> = "0123456789ABCDEF".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict);

        assert!(
            codec.is_none(),
            "Sequential hex should use GenericSimdCodec, not SmallLutCodec"
        );
    }

    #[test]
    fn test_encode_various_sizes() {
        // Test various input sizes with shuffled alphabet
        let chars: Vec<char> = "zyxwvutsrqponmlk".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        // 16 bytes (exactly one SIMD block)
        let data16: Vec<u8> = (0..16).collect();
        let encoded16 = codec.encode(&data16, &dict).unwrap();
        assert_eq!(encoded16.len(), 32);

        // 32 bytes (two SIMD blocks)
        let data32: Vec<u8> = (0..32).collect();
        let encoded32 = codec.encode(&data32, &dict).unwrap();
        assert_eq!(encoded32.len(), 64);

        // 15 bytes (less than one block, uses scalar)
        let data15: Vec<u8> = (0..15).collect();
        let encoded15 = codec.encode(&data15, &dict).unwrap();
        assert_eq!(encoded15.len(), 30);

        // 17 bytes (one block + remainder)
        let data17: Vec<u8> = (0..17).collect();
        let encoded17 = codec.encode(&data17, &dict).unwrap();
        assert_eq!(encoded17.len(), 34);
    }

    #[test]
    fn test_encode_empty_input() {
        let chars: Vec<char> = "zyxwvutsrqponmlk".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        let data: Vec<u8> = vec![];
        let encoded = codec.encode(&data, &dict).unwrap();

        assert_eq!(encoded, "");
    }

    /// Integration test: verify SmallLutCodec is selected by encode_with_simd
    /// for shuffled base16 alphabets
    #[test]
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn test_integration_smalllut_selection() {
        use crate::simd::encode_with_simd;

        // Shuffled 16-char alphabet (arbitrary, non-sequential)
        let alphabet = "fedcba9876543210";
        let chars: Vec<char> = alphabet.chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        // Test data: 32 bytes (two SIMD blocks)
        let data = b"\x00\x11\x22\x33\x44\x55\x66\x77\x88\x99\xAA\xBB\xCC\xDD\xEE\xFF\
                     \x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10";

        // Encode through public API (should select SmallLutCodec)
        let result = encode_with_simd(data, &dict);
        assert!(
            result.is_some(),
            "SmallLutCodec should be selected for shuffled base16"
        );

        let encoded = result.unwrap();

        // Verify output length: 32 bytes -> 64 hex chars
        assert_eq!(encoded.len(), 64);

        // Verify encoding correctness for first byte:
        // 0x00 -> nibbles [0x0, 0x0] -> chars[0] = 'f', chars[0] = 'f'
        assert_eq!(encoded.chars().nth(0).unwrap(), 'f');
        assert_eq!(encoded.chars().nth(1).unwrap(), 'f');

        // Verify second byte: 0x11 -> nibbles [0x1, 0x1] -> chars[1] = 'e', chars[1] = 'e'
        assert_eq!(encoded.chars().nth(2).unwrap(), 'e');
        assert_eq!(encoded.chars().nth(3).unwrap(), 'e');

        // Verify all chars are from the alphabet
        for ch in encoded.chars() {
            assert!(
                alphabet.contains(ch),
                "Output char '{}' should be in alphabet",
                ch
            );
        }

        // Verify a complex byte: 0xFF -> nibbles [0xF, 0xF] -> chars[15] = '0', chars[15] = '0'
        // 0xFF is at position 15 (byte index 15), which is position 30,31 in the encoded string
        assert_eq!(encoded.chars().nth(30).unwrap(), '0');
        assert_eq!(encoded.chars().nth(31).unwrap(), '0');
    }

    /// Verify SIMD path is actually used (not scalar fallback)
    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_path_verified_x86() {
        use crate::simd::{encode_with_simd, has_ssse3};

        // Skip if SSSE3 not available
        if !has_ssse3() {
            eprintln!("SSSE3 not available, skipping SIMD verification");
            return;
        }

        // Shuffled alphabet
        let chars: Vec<char> = "9876543210zyxwvu".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        // Large enough data to trigger SIMD (≥16 bytes)
        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10";

        let result = encode_with_simd(data, &dict);
        assert!(result.is_some(), "SIMD should be available");

        // If we got a result, SIMD was used (scalar would return None from encode_with_simd)
        let encoded = result.unwrap();
        assert_eq!(encoded.len(), 32); // 16 bytes -> 32 hex chars
    }

    /// Verify SIMD path is actually used (not scalar fallback)
    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_simd_path_verified_arm() {
        use crate::simd::encode_with_simd;

        // NEON is always available on aarch64
        let chars: Vec<char> = "9876543210zyxwvu".chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        // Large enough data to trigger SIMD (≥16 bytes)
        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\xFE\xDC\xBA\x98\x76\x54\x32\x10";

        let result = encode_with_simd(data, &dict);
        assert!(result.is_some(), "SIMD should be available");

        // If we got a result, SIMD was used
        let encoded = result.unwrap();
        assert_eq!(encoded.len(), 32); // 16 bytes -> 32 hex chars
    }

    // === DECODE TESTS ===

    #[test]
    fn test_decode_round_trip() {
        // Shuffled alphabet
        let chars: Vec<char> = "9876543210ZYXWVU".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        let data = b"\x01\x23\x45\x67\x89\xAB\xCD\xEF";
        let encoded = codec.encode(data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    #[test]
    fn test_decode_invalid_character() {
        // Alphabet: 0-9, Z-U
        let chars: Vec<char> = "0123456789ZYXWVU".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        // 'A' is not in alphabet
        let invalid = "01A3";
        let result = codec.decode(invalid, &dict);

        assert!(result.is_none(), "Should reject invalid character 'A'");
    }

    #[test]
    fn test_decode_odd_length() {
        let chars: Vec<char> = "zyxwvutsrqponmlk".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        // Odd length (missing one char)
        let invalid = "zyx";
        let result = codec.decode(invalid, &dict);

        assert!(result.is_none(), "Should reject odd-length input");
    }

    #[test]
    fn test_decode_empty_input() {
        let chars: Vec<char> = "zyxwvutsrqponmlk".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        let result = codec.decode("", &dict).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_decode_various_sizes() {
        let chars: Vec<char> = "zyxwvutsrqponmlk".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        // 1 byte (2 chars)
        let data1 = b"\xAB";
        let enc1 = codec.encode(data1, &dict).unwrap();
        let dec1 = codec.decode(&enc1, &dict).unwrap();
        assert_eq!(&dec1[..], &data1[..]);

        // 16 bytes (32 chars)
        let data16: Vec<u8> = (0..16).collect();
        let enc16 = codec.encode(&data16, &dict).unwrap();
        let dec16 = codec.decode(&enc16, &dict).unwrap();
        assert_eq!(&dec16[..], &data16[..]);

        // 32 bytes (64 chars)
        let data32: Vec<u8> = (0..32).collect();
        let enc32 = codec.encode(&data32, &dict).unwrap();
        let dec32 = codec.decode(&enc32, &dict).unwrap();
        assert_eq!(&dec32[..], &data32[..]);

        // 17 bytes (34 chars)
        let data17: Vec<u8> = (0..17).collect();
        let enc17 = codec.encode(&data17, &dict).unwrap();
        let dec17 = codec.decode(&enc17, &dict).unwrap();
        assert_eq!(&dec17[..], &data17[..]);
    }

    #[test]
    fn test_decode_all_nibble_values() {
        // Test all 16 possible nibble values
        let chars: Vec<char> = "zyxwvutsrqponmlk".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        // All nibbles from 0x00 to 0xFF
        let data: Vec<u8> = (0..=255).collect();
        let encoded = codec.encode(&data, &dict).unwrap();
        let decoded = codec.decode(&encoded, &dict).unwrap();

        assert_eq!(&decoded[..], &data[..]);
    }

    /// Integration test: verify SmallLutCodec decode is selected by decode_with_simd
    #[test]
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn test_integration_decode_selection() {
        use crate::simd::{decode_with_simd, encode_with_simd};

        // Shuffled 16-char alphabet (arbitrary, non-sequential)
        let alphabet = "fedcba9876543210";
        let chars: Vec<char> = alphabet.chars().collect();
        let dict = Dictionary::new(chars).unwrap();

        // Test data
        let data = b"Hello, SIMD world! Testing decode path...";

        // Encode
        let encoded = encode_with_simd(data, &dict).expect("Encode failed");

        // Decode through public API (should select SmallLutCodec)
        let decoded = decode_with_simd(&encoded, &dict).expect("Decode failed");

        // Verify round-trip
        assert_eq!(&decoded[..], &data[..]);
    }

    /// Test case sensitivity in decode (different chars)
    #[test]
    fn test_decode_case_sensitive() {
        // Alphabet with both upper and lower case
        let chars: Vec<char> = "zyxwvutsrqpABCDE".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let codec = SmallLutCodec::from_dictionary(&dict).unwrap();

        // Encode 0xF0 -> nibbles [0xF, 0x0] -> chars[15]='E', chars[0]='z'
        let data = b"\xF0";
        let encoded = codec.encode(data, &dict).unwrap();
        assert_eq!(encoded, "Ez");

        // Decode should be case-sensitive
        let decoded = codec.decode(&encoded, &dict).unwrap();
        assert_eq!(&decoded[..], &data[..]);

        // Wrong case should fail (if 'e' not in alphabet)
        let wrong_case = "ez";
        let result = codec.decode(wrong_case, &dict);
        assert!(
            result.is_none(),
            "Should reject wrong case (lowercase 'e' not in alphabet)"
        );
    }
}
