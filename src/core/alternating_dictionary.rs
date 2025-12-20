//! Alternating word dictionary for PGP-style biometric encoding.
//!
//! Provides a dictionary that alternates between multiple sub-dictionaries based on byte position.
//! This is used for PGP biometric word lists where even and odd bytes use different word sets.
//!
//! # Example
//!
//! ```no_run
//! use base_d::{WordDictionary, AlternatingWordDictionary};
//!
//! let even = WordDictionary::builder()
//!     .words(vec!["aardvark", "absurd", "accrue", "acme"])
//!     .build()
//!     .unwrap();
//!
//! let odd = WordDictionary::builder()
//!     .words(vec!["adroitness", "adviser", "aftermath", "aggregate"])
//!     .build()
//!     .unwrap();
//!
//! let alternating = AlternatingWordDictionary::new(
//!     vec![even, odd],
//!     "-".to_string(),
//! );
//!
//! // Byte at position 0 uses even dictionary, position 1 uses odd dictionary, etc.
//! assert_eq!(alternating.encode_byte(0, 0), Some("aardvark")); // Even position
//! assert_eq!(alternating.encode_byte(0, 1), Some("adroitness")); // Odd position
//! ```

use super::word_dictionary::WordDictionary;

/// A word dictionary that alternates between multiple sub-dictionaries.
///
/// Used for PGP biometric word lists where even/odd bytes use different words.
/// Each byte position determines which sub-dictionary is used for encoding/decoding.
#[derive(Debug, Clone)]
pub struct AlternatingWordDictionary {
    dictionaries: Vec<WordDictionary>,
    delimiter: String,
}

impl AlternatingWordDictionary {
    /// Creates a new AlternatingWordDictionary.
    ///
    /// # Parameters
    ///
    /// - `dictionaries`: Vector of WordDictionary instances to alternate between (must be non-empty)
    /// - `delimiter`: String to join encoded words (e.g., "-" or " ")
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `dictionaries` is empty
    /// - Any dictionary does not have exactly 256 words (required for full byte coverage)
    ///
    /// # Example
    ///
    /// ```
    /// use base_d::{WordDictionary, AlternatingWordDictionary};
    ///
    /// // Create dictionaries with 256 words each for full byte coverage
    /// let dict1 = WordDictionary::builder()
    ///     .words((0..256).map(|i| format!("even{}", i)).collect::<Vec<_>>())
    ///     .build()
    ///     .unwrap();
    ///
    /// let dict2 = WordDictionary::builder()
    ///     .words((0..256).map(|i| format!("odd{}", i)).collect::<Vec<_>>())
    ///     .build()
    ///     .unwrap();
    ///
    /// let alternating = AlternatingWordDictionary::new(
    ///     vec![dict1, dict2],
    ///     " ".to_string(),
    /// );
    /// ```
    pub fn new(dictionaries: Vec<WordDictionary>, delimiter: String) -> Self {
        if dictionaries.is_empty() {
            panic!("AlternatingWordDictionary requires at least one sub-dictionary");
        }

        // Validate that all dictionaries have exactly 256 words for full byte coverage
        for (i, dict) in dictionaries.iter().enumerate() {
            if dict.base() != 256 {
                panic!(
                    "Dictionary at index {} has {} words, but exactly 256 words are required for full byte coverage (0-255)",
                    i,
                    dict.base()
                );
            }
        }

        Self {
            dictionaries,
            delimiter,
        }
    }

    /// Returns which dictionary index to use for a given byte position.
    ///
    /// Uses modulo arithmetic to cycle through available dictionaries.
    ///
    /// # Example
    ///
    /// ```
    /// # use base_d::{WordDictionary, AlternatingWordDictionary};
    /// # let dict1 = WordDictionary::builder().words((0..256).map(|i| format!("a{}", i)).collect::<Vec<_>>()).build().unwrap();
    /// # let dict2 = WordDictionary::builder().words((0..256).map(|i| format!("b{}", i)).collect::<Vec<_>>()).build().unwrap();
    /// # let alternating = AlternatingWordDictionary::new(vec![dict1, dict2], " ".to_string());
    /// assert_eq!(alternating.dict_index(0), 0);
    /// assert_eq!(alternating.dict_index(1), 1);
    /// assert_eq!(alternating.dict_index(2), 0);
    /// assert_eq!(alternating.dict_index(3), 1);
    /// ```
    pub fn dict_index(&self, byte_position: usize) -> usize {
        byte_position % self.dictionaries.len()
    }

    /// Get the dictionary for a given byte position.
    ///
    /// # Example
    ///
    /// ```
    /// # use base_d::{WordDictionary, AlternatingWordDictionary};
    /// # let dict1 = WordDictionary::builder().words((0..256).map(|i| format!("a{}", i)).collect::<Vec<_>>()).build().unwrap();
    /// # let dict2 = WordDictionary::builder().words((0..256).map(|i| format!("b{}", i)).collect::<Vec<_>>()).build().unwrap();
    /// # let alternating = AlternatingWordDictionary::new(vec![dict1, dict2], " ".to_string());
    /// let dict_at_0 = alternating.dict_at(0);
    /// let dict_at_1 = alternating.dict_at(1);
    /// // dict_at_0 and dict_at_1 are different dictionaries
    /// ```
    pub fn dict_at(&self, position: usize) -> &WordDictionary {
        &self.dictionaries[self.dict_index(position)]
    }

    /// Encode a single byte at a given position.
    ///
    /// The dictionary used depends on the byte position.
    ///
    /// # Parameters
    ///
    /// - `byte`: The byte value to encode (0-255)
    /// - `position`: The position of this byte in the input stream
    ///
    /// # Returns
    ///
    /// The word corresponding to this byte at this position, or None if the byte value
    /// exceeds the dictionary size.
    ///
    /// # Example
    ///
    /// ```
    /// # use base_d::{WordDictionary, AlternatingWordDictionary};
    /// # let even = WordDictionary::builder().words((0..256).map(|i| format!("even{}", i)).collect::<Vec<_>>()).build().unwrap();
    /// # let odd = WordDictionary::builder().words((0..256).map(|i| format!("odd{}", i)).collect::<Vec<_>>()).build().unwrap();
    /// # let alternating = AlternatingWordDictionary::new(vec![even, odd], " ".to_string());
    /// assert_eq!(alternating.encode_byte(42, 0), Some("even42")); // Even position
    /// assert_eq!(alternating.encode_byte(42, 1), Some("odd42")); // Odd position
    /// ```
    pub fn encode_byte(&self, byte: u8, position: usize) -> Option<&str> {
        self.dict_at(position).encode_word(byte as usize)
    }

    /// Decode a word at a given position back to a byte.
    ///
    /// The dictionary used depends on the word position. Case sensitivity
    /// is determined by the sub-dictionary's case_sensitive setting.
    ///
    /// # Parameters
    ///
    /// - `word`: The word to decode
    /// - `position`: The position of this word in the encoded sequence
    ///
    /// # Returns
    ///
    /// The byte value (0-255) corresponding to this word at this position,
    /// or None if the word is not found in the appropriate dictionary.
    ///
    /// # Example
    ///
    /// ```
    /// # use base_d::{WordDictionary, AlternatingWordDictionary};
    /// # let even = WordDictionary::builder().words((0..256).map(|i| format!("even{}", i)).collect::<Vec<_>>()).build().unwrap();
    /// # let odd = WordDictionary::builder().words((0..256).map(|i| format!("odd{}", i)).collect::<Vec<_>>()).build().unwrap();
    /// # let alternating = AlternatingWordDictionary::new(vec![even, odd], " ".to_string());
    /// // "even0" is index 0 in even dictionary (position 0)
    /// assert_eq!(alternating.decode_word("even0", 0), Some(0));
    /// // "odd1" is index 1 in odd dictionary (position 1)
    /// assert_eq!(alternating.decode_word("odd1", 1), Some(1));
    /// ```
    pub fn decode_word(&self, word: &str, position: usize) -> Option<u8> {
        self.dict_at(position)
            .decode_word(word)
            .map(|idx| idx as u8)
    }

    /// Returns the delimiter used between encoded words.
    pub fn delimiter(&self) -> &str {
        &self.delimiter
    }

    /// Returns the number of sub-dictionaries.
    pub fn num_dicts(&self) -> usize {
        self.dictionaries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_dictionaries() -> Vec<WordDictionary> {
        // Create dictionaries with 256 words each (required by AlternatingWordDictionary)
        let even_words: Vec<String> = (0..256).map(|i| format!("even{}", i)).collect();
        let odd_words: Vec<String> = (0..256).map(|i| format!("odd{}", i)).collect();

        let even = WordDictionary::builder().words(even_words).build().unwrap();

        let odd = WordDictionary::builder().words(odd_words).build().unwrap();

        vec![even, odd]
    }

    #[test]
    fn test_dict_index() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string());

        assert_eq!(alternating.dict_index(0), 0);
        assert_eq!(alternating.dict_index(1), 1);
        assert_eq!(alternating.dict_index(2), 0);
        assert_eq!(alternating.dict_index(3), 1);
    }

    #[test]
    fn test_encode_byte() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string());

        // Position 0 (even) - byte 0
        assert_eq!(alternating.encode_byte(0, 0), Some("even0"));
        // Position 1 (odd) - byte 0
        assert_eq!(alternating.encode_byte(0, 1), Some("odd0"));
        // Position 2 (even) - byte 1
        assert_eq!(alternating.encode_byte(1, 2), Some("even1"));
        // Position 3 (odd) - byte 1
        assert_eq!(alternating.encode_byte(1, 3), Some("odd1"));
    }

    #[test]
    fn test_decode_word() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string());

        // Position 0 (even)
        assert_eq!(alternating.decode_word("even0", 0), Some(0));
        assert_eq!(alternating.decode_word("even1", 0), Some(1));

        // Position 1 (odd)
        assert_eq!(alternating.decode_word("odd0", 1), Some(0));
        assert_eq!(alternating.decode_word("odd1", 1), Some(1));
    }

    #[test]
    fn test_decode_word_case_insensitive() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string());

        assert_eq!(alternating.decode_word("EVEN0", 0), Some(0));
        assert_eq!(alternating.decode_word("EvEn0", 0), Some(0));
        assert_eq!(alternating.decode_word("ODD0", 1), Some(0));
    }

    #[test]
    fn test_decode_word_case_sensitive() {
        // Create 256-word dictionaries with case sensitivity
        let even_words: Vec<String> = (0..256).map(|i| format!("Even{}", i)).collect();
        let odd_words: Vec<String> = (0..256).map(|i| format!("Odd{}", i)).collect();

        let even = WordDictionary::builder()
            .words(even_words)
            .case_sensitive(true)
            .build()
            .unwrap();

        let odd = WordDictionary::builder()
            .words(odd_words)
            .case_sensitive(true)
            .build()
            .unwrap();

        let alternating = AlternatingWordDictionary::new(vec![even, odd], "-".to_string());

        assert_eq!(alternating.decode_word("Even0", 0), Some(0));
        assert_eq!(alternating.decode_word("even0", 0), None);
        assert_eq!(alternating.decode_word("EVEN0", 0), None);
    }

    #[test]
    fn test_delimiter() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string());

        assert_eq!(alternating.delimiter(), "-");
    }

    #[test]
    fn test_num_dicts() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string());

        assert_eq!(alternating.num_dicts(), 2);
    }

    #[test]
    fn test_encode_byte_all_values() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string());

        // Dictionary has 256 words, so all byte values should work
        assert_eq!(alternating.encode_byte(0, 0), Some("even0"));
        assert_eq!(alternating.encode_byte(128, 0), Some("even128"));
        assert_eq!(alternating.encode_byte(255, 0), Some("even255"));
    }

    #[test]
    fn test_decode_word_not_found() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string());

        assert_eq!(alternating.decode_word("unknown", 0), None);
        assert_eq!(alternating.decode_word("unknown", 1), None);
    }

    #[test]
    #[should_panic(expected = "AlternatingWordDictionary requires at least one sub-dictionary")]
    fn test_empty_dictionaries_panics() {
        AlternatingWordDictionary::new(vec![], "-".to_string());
    }

    #[test]
    #[should_panic(expected = "has 4 words, but exactly 256 words are required")]
    fn test_undersized_dictionary_panics() {
        // Create dictionaries with only 4 words each (should panic)
        let even = WordDictionary::builder()
            .words(vec!["aardvark", "absurd", "accrue", "acme"])
            .build()
            .unwrap();

        let odd = WordDictionary::builder()
            .words(vec!["adroitness", "adviser", "aftermath", "aggregate"])
            .build()
            .unwrap();

        // This should panic because dictionaries don't have 256 words
        AlternatingWordDictionary::new(vec![even, odd], "-".to_string());
    }

    #[test]
    fn test_valid_256_word_dictionaries() {
        // Create dictionaries with exactly 256 words - should not panic
        let even_words: Vec<String> = (0..256).map(|i| format!("even{}", i)).collect();
        let odd_words: Vec<String> = (0..256).map(|i| format!("odd{}", i)).collect();

        let even = WordDictionary::builder().words(even_words).build().unwrap();
        let odd = WordDictionary::builder().words(odd_words).build().unwrap();

        let alternating = AlternatingWordDictionary::new(vec![even, odd], "-".to_string());
        assert_eq!(alternating.num_dicts(), 2);
    }
}
