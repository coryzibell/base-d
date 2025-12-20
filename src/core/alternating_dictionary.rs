//! Alternating word dictionary for PGP-style biometric encoding.
//!
//! Provides a dictionary that alternates between multiple sub-dictionaries based on byte position.
//! This is used for PGP biometric word lists where even and odd bytes use different word sets.
//!
//! # Example
//!
//! ```
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
//!     false,
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
    case_sensitive: bool,
}

impl AlternatingWordDictionary {
    /// Creates a new AlternatingWordDictionary.
    ///
    /// # Parameters
    ///
    /// - `dictionaries`: Vector of WordDictionary instances to alternate between
    /// - `delimiter`: String to join encoded words (e.g., "-" or " ")
    /// - `case_sensitive`: Whether decoding should be case-sensitive
    ///
    /// # Example
    ///
    /// ```
    /// use base_d::{WordDictionary, AlternatingWordDictionary};
    ///
    /// let dict1 = WordDictionary::builder()
    ///     .words(vec!["alpha", "bravo"])
    ///     .build()
    ///     .unwrap();
    ///
    /// let dict2 = WordDictionary::builder()
    ///     .words(vec!["one", "two"])
    ///     .build()
    ///     .unwrap();
    ///
    /// let alternating = AlternatingWordDictionary::new(
    ///     vec![dict1, dict2],
    ///     " ".to_string(),
    ///     false,
    /// );
    /// ```
    pub fn new(dictionaries: Vec<WordDictionary>, delimiter: String, case_sensitive: bool) -> Self {
        Self {
            dictionaries,
            delimiter,
            case_sensitive,
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
    /// # let dict1 = WordDictionary::builder().words(vec!["a"]).build().unwrap();
    /// # let dict2 = WordDictionary::builder().words(vec!["b"]).build().unwrap();
    /// # let alternating = AlternatingWordDictionary::new(vec![dict1, dict2], " ".to_string(), false);
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
    /// # let dict1 = WordDictionary::builder().words(vec!["a"]).build().unwrap();
    /// # let dict2 = WordDictionary::builder().words(vec!["b"]).build().unwrap();
    /// # let alternating = AlternatingWordDictionary::new(vec![dict1, dict2], " ".to_string(), false);
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
    /// # let alternating = AlternatingWordDictionary::new(vec![even, odd], " ".to_string(), false);
    /// assert_eq!(alternating.encode_byte(42, 0), Some("even42")); // Even position
    /// assert_eq!(alternating.encode_byte(42, 1), Some("odd42")); // Odd position
    /// ```
    pub fn encode_byte(&self, byte: u8, position: usize) -> Option<&str> {
        self.dict_at(position).encode_word(byte as usize)
    }

    /// Decode a word at a given position back to a byte.
    ///
    /// The dictionary used depends on the word position.
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
    /// # let even = WordDictionary::builder().words(vec!["aardvark", "adroitness"]).build().unwrap();
    /// # let odd = WordDictionary::builder().words(vec!["absurd", "adviser"]).build().unwrap();
    /// # let alternating = AlternatingWordDictionary::new(vec![even, odd], " ".to_string(), false);
    /// // "aardvark" is index 0 in even dictionary (position 0)
    /// assert_eq!(alternating.decode_word("aardvark", 0), Some(0));
    /// // "adviser" is index 1 in odd dictionary (position 1)
    /// assert_eq!(alternating.decode_word("adviser", 1), Some(1));
    /// ```
    pub fn decode_word(&self, word: &str, position: usize) -> Option<u8> {
        let lookup_word = if self.case_sensitive {
            word.to_string()
        } else {
            word.to_lowercase()
        };
        self.dict_at(position)
            .decode_word(&lookup_word)
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

    /// Returns whether this dictionary uses case-sensitive matching.
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_dictionaries() -> Vec<WordDictionary> {
        let even = WordDictionary::builder()
            .words(vec!["aardvark", "absurd", "accrue", "acme"])
            .build()
            .unwrap();

        let odd = WordDictionary::builder()
            .words(vec!["adroitness", "adviser", "aftermath", "aggregate"])
            .build()
            .unwrap();

        vec![even, odd]
    }

    #[test]
    fn test_dict_index() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string(), false);

        assert_eq!(alternating.dict_index(0), 0);
        assert_eq!(alternating.dict_index(1), 1);
        assert_eq!(alternating.dict_index(2), 0);
        assert_eq!(alternating.dict_index(3), 1);
    }

    #[test]
    fn test_encode_byte() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string(), false);

        // Position 0 (even) - byte 0
        assert_eq!(alternating.encode_byte(0, 0), Some("aardvark"));
        // Position 1 (odd) - byte 0
        assert_eq!(alternating.encode_byte(0, 1), Some("adroitness"));
        // Position 2 (even) - byte 1
        assert_eq!(alternating.encode_byte(1, 2), Some("absurd"));
        // Position 3 (odd) - byte 1
        assert_eq!(alternating.encode_byte(1, 3), Some("adviser"));
    }

    #[test]
    fn test_decode_word() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string(), false);

        // Position 0 (even)
        assert_eq!(alternating.decode_word("aardvark", 0), Some(0));
        assert_eq!(alternating.decode_word("absurd", 0), Some(1));

        // Position 1 (odd)
        assert_eq!(alternating.decode_word("adroitness", 1), Some(0));
        assert_eq!(alternating.decode_word("adviser", 1), Some(1));
    }

    #[test]
    fn test_decode_word_case_insensitive() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string(), false);

        assert_eq!(alternating.decode_word("AARDVARK", 0), Some(0));
        assert_eq!(alternating.decode_word("AaRdVaRk", 0), Some(0));
        assert_eq!(alternating.decode_word("ADROITNESS", 1), Some(0));
    }

    #[test]
    fn test_decode_word_case_sensitive() {
        let even = WordDictionary::builder()
            .words(vec!["Aardvark", "Absurd"])
            .case_sensitive(true)
            .build()
            .unwrap();

        let odd = WordDictionary::builder()
            .words(vec!["Adroitness", "Adviser"])
            .case_sensitive(true)
            .build()
            .unwrap();

        let alternating = AlternatingWordDictionary::new(vec![even, odd], "-".to_string(), true);

        assert_eq!(alternating.decode_word("Aardvark", 0), Some(0));
        assert_eq!(alternating.decode_word("aardvark", 0), None);
        assert_eq!(alternating.decode_word("AARDVARK", 0), None);
    }

    #[test]
    fn test_delimiter() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string(), false);

        assert_eq!(alternating.delimiter(), "-");
    }

    #[test]
    fn test_num_dicts() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string(), false);

        assert_eq!(alternating.num_dicts(), 2);
    }

    #[test]
    fn test_encode_byte_out_of_range() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string(), false);

        // Dictionary only has 4 words, so index 4 (byte 4) should return None
        assert_eq!(alternating.encode_byte(4, 0), None);
        assert_eq!(alternating.encode_byte(255, 0), None);
    }

    #[test]
    fn test_decode_word_not_found() {
        let dicts = create_test_dictionaries();
        let alternating = AlternatingWordDictionary::new(dicts, "-".to_string(), false);

        assert_eq!(alternating.decode_word("unknown", 0), None);
        assert_eq!(alternating.decode_word("unknown", 1), None);
    }
}
