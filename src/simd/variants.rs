//! Dictionary metadata and translation strategies for SIMD encoding
//!
//! This module analyzes dictionaries to determine optimal SIMD translation
//! strategies and defines dictionary variants for known encodings.

use crate::core::dictionary::Dictionary;

/// Base64 dictionary variants
///
/// Different base64 standards use different characters at positions 62 and 63:
/// - Standard (RFC 4648): uses '+' and '/'
/// - URL-safe (RFC 4648): uses '-' and '_'
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictionaryVariant {
    /// Standard base64 dictionary: A-Za-z0-9+/
    Base64Standard,
    /// URL-safe base64 dictionary: A-Za-z0-9-_
    Base64Url,
}

/// Base32 dictionary variants
///
/// Different base32 standards use different character sets:
/// - RFC 4648 Standard: A-Z and 2-7
/// - RFC 4648 Extended Hex: 0-9 and A-V
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base32Variant {
    /// RFC 4648 standard: A-Z, 2-7 with padding
    Rfc4648,
    /// RFC 4648 Extended Hex: 0-9, A-V
    Rfc4648Hex,
}

/// Identify which base64 dictionary variant a Dictionary uses
///
/// Returns `None` if the dictionary is not base64 (base != 64) or
/// if it doesn't match a known variant exactly.
///
/// This performs a strict check of all 64 positions to ensure the dictionary
/// matches the standard or URL-safe variant completely.
///
/// # Arguments
///
/// * `dict` - The Dictionary to identify
///
/// # Returns
///
/// - `Some(DictionaryVariant::Base64Standard)` if all positions match standard base64
/// - `Some(DictionaryVariant::Base64Url)` if all positions match URL-safe base64
/// - `None` if not base64 or doesn't match a known variant exactly
pub fn identify_base64_variant(dict: &Dictionary) -> Option<DictionaryVariant> {
    // Only works for base64
    if dict.base() != 64 {
        return None;
    }

    // Check characters at positions 62 and 63 for quick filtering
    let char_62 = dict.encode_digit(62)?;
    let char_63 = dict.encode_digit(63)?;

    let candidate = match (char_62, char_63) {
        ('+', '/') => DictionaryVariant::Base64Standard,
        ('-', '_') => DictionaryVariant::Base64Url,
        _ => return None,
    };

    // Verify all positions match the expected variant
    if verify_base64_dictionary(dict, candidate) {
        Some(candidate)
    } else {
        None
    }
}

/// Verify that a dictionary matches the expected base64 dictionary variant
///
/// Checks all 64 positions to ensure they match the expected dictionary.
///
/// # Arguments
///
/// * `dict` - The Dictionary to verify
/// * `variant` - The expected dictionary variant
///
/// # Returns
///
/// `true` if all positions match, `false` otherwise
pub fn verify_base64_dictionary(dict: &Dictionary, variant: DictionaryVariant) -> bool {
    if dict.base() != 64 {
        return false;
    }

    let expected = match variant {
        DictionaryVariant::Base64Standard => {
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        }
        DictionaryVariant::Base64Url => {
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
        }
    };

    for (i, expected_char) in expected.chars().enumerate() {
        if dict.encode_digit(i) != Some(expected_char) {
            return false;
        }
    }

    true
}

/// Identify which base32 dictionary variant a Dictionary uses
///
/// Returns `None` if the dictionary is not base32 (base != 32) or
/// if it doesn't match a known variant exactly.
///
/// # Arguments
///
/// * `dict` - The Dictionary to identify
///
/// # Returns
///
/// - `Some(Base32Variant::Rfc4648)` if all positions match RFC 4648 standard
/// - `Some(Base32Variant::Rfc4648Hex)` if all positions match RFC 4648 extended hex
/// - `None` if not base32 or doesn't match a known variant exactly
pub fn identify_base32_variant(dict: &Dictionary) -> Option<Base32Variant> {
    // Only works for base32
    if dict.base() != 32 {
        return None;
    }

    // Check character patterns
    let chars: Vec<char> = (0..32).filter_map(|i| dict.encode_digit(i)).collect();
    if chars.len() != 32 {
        return None;
    }

    // RFC 4648: A-Z, 2-7 (positions 0-25 = A-Z, 26-31 = 2-7)
    if chars[0] == 'A' && chars[25] == 'Z' && chars[26] == '2' && chars[31] == '7' {
        let candidate = Base32Variant::Rfc4648;
        if verify_base32_dictionary(dict, candidate) {
            return Some(candidate);
        }
    }

    // RFC 4648 Hex: 0-9, A-V (positions 0-9 = 0-9, 10-31 = A-V)
    if chars[0] == '0' && chars[9] == '9' && chars[10] == 'A' && chars[31] == 'V' {
        let candidate = Base32Variant::Rfc4648Hex;
        if verify_base32_dictionary(dict, candidate) {
            return Some(candidate);
        }
    }

    None
}

/// Verify that a dictionary matches the expected base32 dictionary variant
///
/// Checks all 32 positions to ensure they match the expected dictionary.
///
/// # Arguments
///
/// * `dict` - The Dictionary to verify
/// * `variant` - The expected dictionary variant
///
/// # Returns
///
/// `true` if all positions match, `false` otherwise
pub fn verify_base32_dictionary(dict: &Dictionary, variant: Base32Variant) -> bool {
    if dict.base() != 32 {
        return false;
    }

    let expected = match variant {
        Base32Variant::Rfc4648 => "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567",
        Base32Variant::Rfc4648Hex => "0123456789ABCDEFGHIJKLMNOPQRSTUV",
    };

    for (i, expected_char) in expected.chars().enumerate() {
        if dict.encode_digit(i) != Some(expected_char) {
            return false;
        }
    }

    true
}

/// Character range mapping for ranged translation strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharRange {
    /// Index range: [start_index, end_index)
    pub index_start: u8,
    pub index_end: u8,
    /// Character range: [start_char, end_char)
    pub char_start: char,
    pub char_end: char,
    /// Offset to add: char = index + offset
    pub offset: i8,
}

/// Classification of how a dictionary's indices map to characters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationStrategy {
    /// Single contiguous Unicode range: index → char is `start + index`
    /// Example: 64 chars starting at U+0040 ('@')
    /// Encoding: add constant offset
    /// Decoding: subtract constant offset
    Sequential { start_codepoint: u32 },

    /// Multiple contiguous ranges with gaps
    /// Example: base64 (A-Z, a-z, 0-9, +/)
    /// Encoding: range check + offset per range
    /// Decoding: range check + offset per range
    Ranged { ranges: &'static [CharRange] },

    /// Arbitrary mapping requiring lookup table
    /// Example: custom shuffled dictionary
    /// Encoding: LUT required
    /// Decoding: reverse LUT/HashMap required
    Arbitrary { dictionary_size: usize },
}

/// LUT codec selection strategy based on dictionary size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LutStrategy {
    /// Not applicable (sequential or ranged dictionary)
    NotApplicable,
    /// Small dictionary (<=16 chars): Direct pshufb/tbl lookup
    SmallDirect,
    /// Large dictionary (17-64 chars): Platform-dependent (vqtbl4q/vpermb/range-reduction)
    LargePlatformDependent,
    /// Very large (>64 chars): No SIMD benefit, scalar only
    ScalarOnly,
}

/// Known range definitions for standard dictionaries
static BASE64_STANDARD_RANGES: &[CharRange] = &[
    CharRange {
        index_start: 0,
        index_end: 26,
        char_start: 'A',
        char_end: 'Z',
        offset: 65,
    },
    CharRange {
        index_start: 26,
        index_end: 52,
        char_start: 'a',
        char_end: 'z',
        offset: 71,
    },
    CharRange {
        index_start: 52,
        index_end: 62,
        char_start: '0',
        char_end: '9',
        offset: -4,
    },
    // Special cases for +/ handled separately in SIMD
];

static BASE64_URL_RANGES: &[CharRange] = &[
    CharRange {
        index_start: 0,
        index_end: 26,
        char_start: 'A',
        char_end: 'Z',
        offset: 65,
    },
    CharRange {
        index_start: 26,
        index_end: 52,
        char_start: 'a',
        char_end: 'z',
        offset: 71,
    },
    CharRange {
        index_start: 52,
        index_end: 62,
        char_start: '0',
        char_end: '9',
        offset: -4,
    },
    // Special cases for -_ handled separately in SIMD
];

static HEX_UPPER_RANGES: &[CharRange] = &[
    CharRange {
        index_start: 0,
        index_end: 10,
        char_start: '0',
        char_end: '9',
        offset: 48,
    },
    CharRange {
        index_start: 10,
        index_end: 16,
        char_start: 'A',
        char_end: 'F',
        offset: 55,
    },
];

static HEX_LOWER_RANGES: &[CharRange] = &[
    CharRange {
        index_start: 0,
        index_end: 10,
        char_start: '0',
        char_end: '9',
        offset: 48,
    },
    CharRange {
        index_start: 10,
        index_end: 16,
        char_start: 'a',
        char_end: 'f',
        offset: 87,
    },
];

/// Metadata about a dictionary's structure for SIMD optimization
#[derive(Debug, Clone)]
pub struct DictionaryMetadata {
    /// Dictionary base (2, 4, 8, 16, 32, 64, 128, 256)
    pub base: usize,

    /// Bits per symbol (1, 2, 3, 4, 5, 6, 7, 8)
    pub bits_per_symbol: u8,

    /// Translation strategy
    pub strategy: TranslationStrategy,

    /// Whether SIMD acceleration is available
    pub simd_compatible: bool,
}

impl DictionaryMetadata {
    /// Returns whether SIMD acceleration is available for this dictionary
    pub fn simd_available(&self) -> bool {
        self.simd_compatible
    }

    /// Determine LUT codec suitability for arbitrary dictionaries
    pub fn lut_strategy(&self) -> LutStrategy {
        match self.strategy {
            TranslationStrategy::Arbitrary { dictionary_size } => {
                if dictionary_size <= 16 {
                    LutStrategy::SmallDirect
                } else if dictionary_size <= 64 {
                    LutStrategy::LargePlatformDependent
                } else {
                    LutStrategy::ScalarOnly
                }
            }
            _ => LutStrategy::NotApplicable,
        }
    }

    /// Analyze a Dictionary and determine its translation strategy
    pub fn from_dictionary(dict: &Dictionary) -> Self {
        let base = dict.base();

        // Check power-of-2 requirement
        if !base.is_power_of_two() {
            return Self {
                base,
                bits_per_symbol: 0,
                strategy: TranslationStrategy::Arbitrary {
                    dictionary_size: base,
                },
                simd_compatible: false,
            };
        }

        let bits_per_symbol = (base as f64).log2() as u8;

        // Analyze character sequence
        let strategy = Self::detect_strategy(dict);

        // SIMD compatible if:
        // 1. Power of 2 base
        // 2. Sequential or known ranged pattern
        // 3. Base supported by existing SIMD (4, 5, 6, 8 bits)
        // 4. Sequential dictionaries must fit ENTIRELY in one byte:
        //    SequentialTranslate applies the offset with a single `paddb`, so
        //    every codepoint it can represent has to be <= U+00FF -- not just
        //    the start. The constraint is on the whole symbol range, because
        //    the last symbol is `start + 2^bits - 1`. Checking only the start
        //    lets a dictionary that begins inside Latin-1 and runs over the
        //    top (e.g. U+00F8..U+0107) take this path and emit NUL bytes and
        //    control characters where the range wrapped. Nothing in the
        //    registry straddles U+00FF today, so that class is latent -- and
        //    invisible to any sweep over the real dictionaries.
        let simd_compatible = matches!(bits_per_symbol, 4 | 5 | 6 | 8)
            && !matches!(strategy, TranslationStrategy::Arbitrary { .. })
            && !matches!(
                strategy,
                TranslationStrategy::Sequential { start_codepoint }
                    if start_codepoint + (1u32 << bits_per_symbol) - 1 > 0xFF
            );

        Self {
            base,
            bits_per_symbol,
            strategy,
            simd_compatible,
        }
    }

    fn detect_strategy(dict: &Dictionary) -> TranslationStrategy {
        let base = dict.base();
        let chars: Vec<char> = (0..base).filter_map(|i| dict.encode_digit(i)).collect();

        if chars.len() != base {
            return TranslationStrategy::Arbitrary {
                dictionary_size: chars.len(),
            };
        }

        // Check for sequential (all codepoints contiguous)
        let first_codepoint = chars[0] as u32;
        let is_sequential = chars
            .iter()
            .enumerate()
            .all(|(i, &c)| (c as u32) == first_codepoint + (i as u32));

        if is_sequential {
            return TranslationStrategy::Sequential {
                start_codepoint: first_codepoint,
            };
        }

        // Check for known ranged patterns
        if let Some(ranges) = Self::detect_ranges(&chars) {
            return TranslationStrategy::Ranged { ranges };
        }

        TranslationStrategy::Arbitrary {
            dictionary_size: base,
        }
    }

    fn detect_ranges(chars: &[char]) -> Option<&'static [CharRange]> {
        // Check against known patterns

        // Standard base64: A-Za-z0-9+/
        if chars.len() == 64 && Self::matches_base64_standard(chars) {
            return Some(BASE64_STANDARD_RANGES);
        }

        // URL-safe base64: A-Za-z0-9-_
        if chars.len() == 64 && Self::matches_base64_url(chars) {
            return Some(BASE64_URL_RANGES);
        }

        // Hex uppercase: 0-9A-F
        if chars.len() == 16 && Self::matches_hex_upper(chars) {
            return Some(HEX_UPPER_RANGES);
        }

        // Hex lowercase: 0-9a-f
        if chars.len() == 16 && Self::matches_hex_lower(chars) {
            return Some(HEX_LOWER_RANGES);
        }

        None
    }

    fn matches_base64_standard(chars: &[char]) -> bool {
        let expected = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        chars.iter().zip(expected.chars()).all(|(a, b)| *a == b)
    }

    fn matches_base64_url(chars: &[char]) -> bool {
        let expected = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        chars.iter().zip(expected.chars()).all(|(a, b)| *a == b)
    }

    fn matches_hex_upper(chars: &[char]) -> bool {
        let expected = "0123456789ABCDEF";
        chars.iter().zip(expected.chars()).all(|(a, b)| *a == b)
    }

    fn matches_hex_lower(chars: &[char]) -> bool {
        let expected = "0123456789abcdef";
        chars.iter().zip(expected.chars()).all(|(a, b)| *a == b)
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::core::config::EncodingMode;

    fn make_base64_standard_dict() -> Dictionary {
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
            .chars()
            .collect();
        Dictionary::new_with_mode(chars, EncodingMode::Chunked, Some('=')).unwrap()
    }

    fn make_base64_url_dict() -> Dictionary {
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
            .chars()
            .collect();
        Dictionary::new_with_mode(chars, EncodingMode::Chunked, Some('=')).unwrap()
    }

    #[test]
    fn test_identify_standard_base64() {
        let dict = make_base64_standard_dict();
        assert_eq!(
            identify_base64_variant(&dict),
            Some(DictionaryVariant::Base64Standard)
        );
    }

    #[test]
    fn test_identify_base64_url() {
        let dict = make_base64_url_dict();
        assert_eq!(
            identify_base64_variant(&dict),
            Some(DictionaryVariant::Base64Url)
        );
    }

    #[test]
    fn test_identify_non_base64() {
        let chars: Vec<char> = "0123456789ABCDEF".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        assert_eq!(identify_base64_variant(&dict), None);
    }

    #[test]
    fn test_identify_unknown_variant() {
        // Custom base64 with different chars at positions 62-63
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@$"
            .chars()
            .collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();
        assert_eq!(identify_base64_variant(&dict), None);
    }

    #[test]
    fn test_verify_standard_dictionary() {
        let dict = make_base64_standard_dict();
        assert!(verify_base64_dictionary(
            &dict,
            DictionaryVariant::Base64Standard
        ));
        assert!(!verify_base64_dictionary(
            &dict,
            DictionaryVariant::Base64Url
        ));
    }

    #[test]
    fn test_verify_url_dictionary() {
        let dict = make_base64_url_dict();
        assert!(verify_base64_dictionary(
            &dict,
            DictionaryVariant::Base64Url
        ));
        assert!(!verify_base64_dictionary(
            &dict,
            DictionaryVariant::Base64Standard
        ));
    }

    #[test]
    fn test_sequential_dictionary_detection() {
        // Create a sequential dictionary: 64 chars starting at Latin Extended-A (U+0100)
        let chars: Vec<char> = (0x100..0x140)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();

        let metadata = DictionaryMetadata::from_dictionary(&dict);
        assert_eq!(metadata.base, 64);
        assert_eq!(metadata.bits_per_symbol, 6);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Sequential {
                start_codepoint: 0x100
            }
        ));
        // Above the U+00FF ceiling that SequentialTranslate's single-byte add
        // can represent, so SIMD is refused and the scalar path is used.
        assert!(!metadata.simd_compatible);
    }

    #[test]
    fn test_ranged_base64_standard_detection() {
        let dict = make_base64_standard_dict();
        let metadata = DictionaryMetadata::from_dictionary(&dict);

        assert_eq!(metadata.base, 64);
        assert_eq!(metadata.bits_per_symbol, 6);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Ranged { .. }
        ));
        assert!(metadata.simd_compatible);
    }

    #[test]
    fn test_ranged_base64_url_detection() {
        let dict = make_base64_url_dict();
        let metadata = DictionaryMetadata::from_dictionary(&dict);

        assert_eq!(metadata.base, 64);
        assert_eq!(metadata.bits_per_symbol, 6);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Ranged { .. }
        ));
        assert!(metadata.simd_compatible);
    }

    #[test]
    fn test_ranged_hex_upper_detection() {
        let chars: Vec<char> = "0123456789ABCDEF".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let metadata = DictionaryMetadata::from_dictionary(&dict);

        assert_eq!(metadata.base, 16);
        assert_eq!(metadata.bits_per_symbol, 4);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Ranged { .. }
        ));
        assert!(metadata.simd_compatible);
    }

    #[test]
    fn test_ranged_hex_lower_detection() {
        let chars: Vec<char> = "0123456789abcdef".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let metadata = DictionaryMetadata::from_dictionary(&dict);

        assert_eq!(metadata.base, 16);
        assert_eq!(metadata.bits_per_symbol, 4);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Ranged { .. }
        ));
        assert!(metadata.simd_compatible);
    }

    #[test]
    fn test_arbitrary_dictionary_detection() {
        // Create a shuffled/arbitrary dictionary
        let chars: Vec<char> = "ZYXWVUTSRQPONMLKJIHGFEDCBAzyxwvutsrqponmlkjihgfedcba9876543210+/"
            .chars()
            .collect();
        let dict = Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap();
        let metadata = DictionaryMetadata::from_dictionary(&dict);

        assert_eq!(metadata.base, 64);
        assert_eq!(metadata.bits_per_symbol, 6);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Arbitrary {
                dictionary_size: 64
            }
        ));
        assert!(!metadata.simd_compatible);
    }

    #[test]
    fn test_non_power_of_two_detection() {
        // Base 10 is not power of 2
        let chars: Vec<char> = "0123456789".chars().collect();
        let dict = Dictionary::new(chars).unwrap();
        let metadata = DictionaryMetadata::from_dictionary(&dict);

        assert_eq!(metadata.base, 10);
        assert_eq!(metadata.bits_per_symbol, 0);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Arbitrary {
                dictionary_size: 10
            }
        ));
        assert!(!metadata.simd_compatible);
    }

    #[test]
    fn test_base32_sequential() {
        // Create sequential base32 dictionary
        let chars: Vec<char> = (0x41..0x61).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();
        let metadata = DictionaryMetadata::from_dictionary(&dict);

        assert_eq!(metadata.base, 32);
        assert_eq!(metadata.bits_per_symbol, 5);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Sequential {
                start_codepoint: 0x41
            }
        ));
        // base32 (5 bits) is now SIMD compatible
        assert!(metadata.simd_compatible);
    }

    #[test]
    fn test_base256_sequential() {
        // Create sequential base256 dictionary using valid Unicode range
        // Use a range starting from 0x100 (Latin Extended-A) to avoid control characters
        let chars: Vec<char> = (0x100..0x200)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();
        let dict = Dictionary::new(chars).unwrap();
        let metadata = DictionaryMetadata::from_dictionary(&dict);

        assert_eq!(metadata.base, 256);
        assert_eq!(metadata.bits_per_symbol, 8);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Sequential {
                start_codepoint: 0x100
            }
        ));
        // Above the U+00FF ceiling — see test_sequential_dictionary_detection.
        assert!(!metadata.simd_compatible);
    }

    /// Build a Chunked dictionary over `range`. Chunked is required for the
    /// public encode/decode path to dispatch through SIMD at all.
    fn seq_dict(range: std::ops::Range<u32>) -> Dictionary {
        let chars: Vec<char> = range.map(|cp| char::from_u32(cp).unwrap()).collect();
        Dictionary::new_with_mode(chars, EncodingMode::Chunked, None).unwrap()
    }

    /// Assert that a sequential dictionary over `range` never emits a byte
    /// that the codepoint ceiling would have truncated.
    ///
    /// **This is the ceiling property in isolation.** When a symbol range
    /// spills past U+00FF and still takes `SequentialTranslate`, the `paddb`
    /// offset wraps and the encoder emits NUL bytes and control characters.
    /// The property holds at every length and on either path, so unlike a
    /// round trip it is a clean signal for this specific defect.
    ///
    /// Metadata assertions cannot catch a wrong gate on their own: a
    /// misclassified dictionary still reports the strategy and start
    /// codepoint a test expects, and only the emitted bytes show the wrap.
    fn assert_no_truncated_symbols(range: std::ops::Range<u32>) {
        let start = range.start;
        let dict = seq_dict(range);
        for len in 1..=80usize {
            let data: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(37)).collect();
            let encoded = crate::encode(&data, &dict);
            assert!(
                !encoded.chars().any(|c| c == '\0' || c.is_control()),
                "U+{:04X}.. emitted NUL/control characters at len {} -- the \
                 symbol range wrapped past U+00FF",
                start,
                len
            );
        }
    }

    /// Assert a full encode/decode round trip at every one of `lengths`.
    fn assert_roundtrips_at(range: std::ops::Range<u32>, lengths: &[usize]) {
        let start = range.start;
        let dict = seq_dict(range);
        for &len in lengths {
            let data: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(37)).collect();
            let encoded = crate::encode(&data, &dict);
            let decoded = crate::decode(&encoded, &dict).unwrap_or_else(|e| {
                panic!("U+{:04X}.. decode failed at len {}: {:?}", start, len, e)
            });
            assert_eq!(
                decoded, data,
                "U+{:04X}.. round trip failed at len {}",
                start, len
            );
        }
    }

    /// Lengths at which the 4-bit SIMD path is known to round-trip.
    ///
    /// A SEPARATE, PRE-EXISTING defect -- block-count truncation, not the
    /// codepoint ceiling and not fixed by this change -- makes the 4-bit
    /// vector path drop trailing data at most input lengths.
    ///
    /// Swept 1..=80 bytes: the ONLY lengths that round trip are 1..=16, 32
    /// and 64. All 62 others fail, 48 included -- so it is not simply
    /// "multiples of the block size". Measured identically for a plain
    /// ASCII-range dictionary (U+0030) and a Latin-1 one (U+00F0), so the
    /// defect is length-dependent rather than codepoint-dependent, and it
    /// reproduces unchanged on `origin/main`.
    ///
    /// Dictionaries the ceiling gate REJECTS take the scalar path and round
    /// trip at every one of the 80 lengths. That contrast is the point: this
    /// list is short because of the OTHER defect, not this one.
    const SIMD_SAFE_LENGTHS: &[usize] = &[8, 16, 32, 64];

    /// Every length from 1 to 80. All of them pass on the scalar path.
    fn all_lengths() -> Vec<usize> {
        (1..=80).collect()
    }

    #[test]
    fn test_sequential_codepoint_ceiling() {
        // SequentialTranslate adds the start codepoint with a single `paddb`,
        // so the whole symbol range must fit in a byte. Straddle the boundary.
        let at_ceiling: Vec<char> = (0xF0..0x100)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();
        let dict = Dictionary::new(at_ceiling).unwrap();
        let metadata = DictionaryMetadata::from_dictionary(&dict);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Sequential {
                start_codepoint: 0xF0
            }
        ));
        // U+00F0 + 16 - 1 == U+00FF: the last symbol lands exactly on the
        // ceiling, so this one is genuinely representable.
        assert!(metadata.simd_compatible);
        assert_no_truncated_symbols(0xF0..0x100);
        assert_roundtrips_at(0xF0..0x100, SIMD_SAFE_LENGTHS);

        let over_ceiling: Vec<char> = (0x100..0x110)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();
        let dict = Dictionary::new(over_ceiling).unwrap();
        let metadata = DictionaryMetadata::from_dictionary(&dict);
        assert!(!metadata.simd_compatible);
        assert_no_truncated_symbols(0x100..0x110);
        // Gated, so it runs on the scalar path -- correct at EVERY length.
        assert_roundtrips_at(0x100..0x110, &all_lengths());
    }

    #[test]
    fn test_sequential_range_spilling_past_ceiling_is_gated() {
        // The constraint is on the whole symbol range, not on the start. A
        // dictionary that BEGINS inside Latin-1 and RUNS OVER U+00FF passed
        // the old `start_codepoint > 0xFF` gate, took SequentialTranslate,
        // and emitted NUL bytes where the range wrapped.
        //
        // These ranges are synthetic on purpose: nothing in the registry
        // straddles U+00FF, so no sweep over the real dictionaries -- however
        // many dictionaries, lengths, and seeds it covers -- can reach this
        // case. It is latent, and only a constructed range exercises it.
        //
        // 4 bits/symbol throughout, which keeps the separate 6-bit
        // block-count truncation defect out of the result.
        for start in [0xF1u32, 0xF8, 0xFC] {
            let chars: Vec<char> = (start..start + 16)
                .map(|cp| char::from_u32(cp).unwrap())
                .collect();
            let dict = Dictionary::new(chars).unwrap();
            let metadata = DictionaryMetadata::from_dictionary(&dict);

            assert!(
                matches!(metadata.strategy, TranslationStrategy::Sequential { .. }),
                "U+{:04X}.. should still classify as sequential",
                start
            );
            assert!(
                !metadata.simd_compatible,
                "U+{:04X}..U+{:04X} spills past the U+00FF ceiling and must \
                 not take SequentialTranslate",
                start,
                start + 15
            );

            // Having been gated, it must actually produce correct output: no
            // wrapped symbols, and a clean round trip at every length,
            // because the scalar path has neither defect.
            assert_no_truncated_symbols(start..start + 16);
            assert_roundtrips_at(start..start + 16, &all_lengths());
        }
    }

    #[test]
    fn test_sequential_starting_at_printable() {
        // Base16 sequential starting at '!' (U+0021)
        let chars: Vec<char> = (0x21..0x31).map(|cp| char::from_u32(cp).unwrap()).collect();
        let dict = Dictionary::new(chars).unwrap();
        let metadata = DictionaryMetadata::from_dictionary(&dict);

        assert_eq!(metadata.base, 16);
        assert_eq!(metadata.bits_per_symbol, 4);
        assert!(matches!(
            metadata.strategy,
            TranslationStrategy::Sequential {
                start_codepoint: 0x21
            }
        ));
        assert!(metadata.simd_compatible);
    }
}
