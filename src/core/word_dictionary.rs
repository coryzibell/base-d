use std::collections::HashMap;

/// A word-based dictionary for encoding binary data as word sequences.
///
/// Unlike character-based `Dictionary`, this uses whole words as encoding symbols.
/// Designed for BIP-39, Diceware, PGP word lists, and custom word-based encodings.
///
/// # Example
///
/// ```
/// use base_d::WordDictionary;
///
/// let dict = WordDictionary::builder()
///     .words(vec!["abandon", "ability", "able", "about"])
///     .delimiter(" ")
///     .build()
///     .unwrap();
///
/// assert_eq!(dict.base(), 4);
/// assert_eq!(dict.encode_word(0), Some("abandon"));
/// assert_eq!(dict.decode_word("ability"), Some(1));
/// ```
#[derive(Debug, Clone)]
pub struct WordDictionary {
    words: Vec<String>,
    word_to_index: HashMap<String, usize>,
    delimiter: String,
    case_sensitive: bool,
}

impl WordDictionary {
    /// Creates a new WordDictionaryBuilder for constructing a WordDictionary.
    pub fn builder() -> WordDictionaryBuilder {
        WordDictionaryBuilder::new()
    }

    /// Returns the base (number of words) in this dictionary.
    pub fn base(&self) -> usize {
        self.words.len()
    }

    /// Returns the delimiter used between words in encoded output.
    pub fn delimiter(&self) -> &str {
        &self.delimiter
    }

    /// Returns whether this dictionary uses case-sensitive matching.
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Encodes a digit (0 to base-1) as a word.
    ///
    /// Returns `None` if the index is out of range.
    pub fn encode_word(&self, index: usize) -> Option<&str> {
        self.words.get(index).map(|s| s.as_str())
    }

    /// Decodes a word back to its index value.
    ///
    /// Returns `None` if the word is not in the dictionary.
    /// Matching respects the `case_sensitive` setting.
    pub fn decode_word(&self, word: &str) -> Option<usize> {
        let key = if self.case_sensitive {
            word.to_string()
        } else {
            word.to_lowercase()
        };
        self.word_to_index.get(&key).copied()
    }

    /// Returns an iterator over all words in the dictionary.
    pub fn words(&self) -> impl Iterator<Item = &str> {
        self.words.iter().map(|s| s.as_str())
    }
}

/// Builder for constructing a WordDictionary with flexible configuration.
///
/// # Example
///
/// ```
/// use base_d::WordDictionary;
///
/// let dict = WordDictionary::builder()
///     .words(vec!["alpha", "bravo", "charlie", "delta"])
///     .delimiter("-")
///     .case_sensitive(false)
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Default)]
pub struct WordDictionaryBuilder {
    words: Option<Vec<String>>,
    delimiter: Option<String>,
    case_sensitive: Option<bool>,
}

impl WordDictionaryBuilder {
    /// Creates a new WordDictionaryBuilder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the word list from a vector of strings.
    pub fn words<I, S>(mut self, words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.words = Some(words.into_iter().map(|s| s.into()).collect());
        self
    }

    /// Sets the word list from a newline-separated string.
    ///
    /// Empty lines are ignored. Leading/trailing whitespace is trimmed.
    pub fn words_from_str(mut self, s: &str) -> Self {
        self.words = Some(
            s.lines()
                .map(|line| line.trim())
                .filter(|line| !line.is_empty())
                .map(|line| line.to_string())
                .collect(),
        );
        self
    }

    /// Sets the delimiter used between words in encoded output.
    ///
    /// Default is a single space " ".
    pub fn delimiter<S: Into<String>>(mut self, delimiter: S) -> Self {
        self.delimiter = Some(delimiter.into());
        self
    }

    /// Sets whether word matching is case-sensitive.
    ///
    /// Default is false (case-insensitive).
    pub fn case_sensitive(mut self, case_sensitive: bool) -> Self {
        self.case_sensitive = Some(case_sensitive);
        self
    }

    /// Builds the WordDictionary with the configured settings.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No words were provided
    /// - The word list is empty
    /// - Duplicate words exist (considering case sensitivity)
    pub fn build(self) -> Result<WordDictionary, String> {
        let words = self.words.ok_or("No words provided")?;

        if words.is_empty() {
            return Err("Word list cannot be empty".to_string());
        }

        let case_sensitive = self.case_sensitive.unwrap_or(false);
        let delimiter = self.delimiter.unwrap_or_else(|| " ".to_string());

        // Build the reverse lookup, checking for duplicates
        let mut word_to_index = HashMap::with_capacity(words.len());
        for (i, word) in words.iter().enumerate() {
            let key = if case_sensitive {
                word.clone()
            } else {
                word.to_lowercase()
            };

            if word_to_index.insert(key.clone(), i).is_some() {
                return Err(format!(
                    "Duplicate word in dictionary: '{}' (normalized: '{}')",
                    word, key
                ));
            }
        }

        Ok(WordDictionary {
            words,
            word_to_index,
            delimiter,
            case_sensitive,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_word_dictionary() {
        let dict = WordDictionary::builder()
            .words(vec!["abandon", "ability", "able", "about"])
            .build()
            .unwrap();

        assert_eq!(dict.base(), 4);
        assert_eq!(dict.delimiter(), " ");
        assert!(!dict.case_sensitive());
    }

    #[test]
    fn test_encode_word() {
        let dict = WordDictionary::builder()
            .words(vec!["alpha", "bravo", "charlie"])
            .build()
            .unwrap();

        assert_eq!(dict.encode_word(0), Some("alpha"));
        assert_eq!(dict.encode_word(1), Some("bravo"));
        assert_eq!(dict.encode_word(2), Some("charlie"));
        assert_eq!(dict.encode_word(3), None);
    }

    #[test]
    fn test_decode_word_case_insensitive() {
        let dict = WordDictionary::builder()
            .words(vec!["Alpha", "Bravo", "Charlie"])
            .case_sensitive(false)
            .build()
            .unwrap();

        assert_eq!(dict.decode_word("alpha"), Some(0));
        assert_eq!(dict.decode_word("ALPHA"), Some(0));
        assert_eq!(dict.decode_word("Alpha"), Some(0));
        assert_eq!(dict.decode_word("delta"), None);
    }

    #[test]
    fn test_decode_word_case_sensitive() {
        let dict = WordDictionary::builder()
            .words(vec!["Alpha", "Bravo", "Charlie"])
            .case_sensitive(true)
            .build()
            .unwrap();

        assert_eq!(dict.decode_word("Alpha"), Some(0));
        assert_eq!(dict.decode_word("alpha"), None);
        assert_eq!(dict.decode_word("ALPHA"), None);
    }

    #[test]
    fn test_custom_delimiter() {
        let dict = WordDictionary::builder()
            .words(vec!["one", "two", "three"])
            .delimiter("-")
            .build()
            .unwrap();

        assert_eq!(dict.delimiter(), "-");
    }

    #[test]
    fn test_words_from_str() {
        let word_list = "abandon\nability\nable\nabout";
        let dict = WordDictionary::builder()
            .words_from_str(word_list)
            .build()
            .unwrap();

        assert_eq!(dict.base(), 4);
        assert_eq!(dict.encode_word(0), Some("abandon"));
    }

    #[test]
    fn test_words_from_str_with_whitespace() {
        let word_list = "  abandon  \n\n  ability  \n  able  \n\n";
        let dict = WordDictionary::builder()
            .words_from_str(word_list)
            .build()
            .unwrap();

        assert_eq!(dict.base(), 3);
        assert_eq!(dict.encode_word(0), Some("abandon"));
    }

    #[test]
    fn test_empty_word_list_error() {
        let result = WordDictionary::builder()
            .words(Vec::<String>::new())
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_no_words_error() {
        let result = WordDictionary::builder().build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No words"));
    }

    #[test]
    fn test_duplicate_words_error() {
        let result = WordDictionary::builder()
            .words(vec!["alpha", "bravo", "alpha"])
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate"));
    }

    #[test]
    fn test_duplicate_words_case_insensitive() {
        let result = WordDictionary::builder()
            .words(vec!["Alpha", "ALPHA"])
            .case_sensitive(false)
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate"));
    }

    #[test]
    fn test_duplicate_words_case_sensitive_allowed() {
        // With case sensitivity, "Alpha" and "ALPHA" are different
        let result = WordDictionary::builder()
            .words(vec!["Alpha", "ALPHA"])
            .case_sensitive(true)
            .build();
        assert!(result.is_ok());
        let dict = result.unwrap();
        assert_eq!(dict.base(), 2);
    }

    #[test]
    fn test_words_iterator() {
        let dict = WordDictionary::builder()
            .words(vec!["a", "b", "c"])
            .build()
            .unwrap();

        let words: Vec<&str> = dict.words().collect();
        assert_eq!(words, vec!["a", "b", "c"]);
    }
}
