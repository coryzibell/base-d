use crate::core::config::{DictionaryRegistry, EncodingMode};
use crate::core::dictionary::Dictionary;
use crate::decode;
use std::collections::HashSet;

/// A match result from dictionary detection.
#[derive(Debug, Clone)]
pub struct DictionaryMatch {
    /// Name of the matched dictionary
    pub name: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// The dictionary itself
    pub dictionary: Dictionary,
}

/// Detector for automatically identifying which dictionary was used to encode data.
pub struct DictionaryDetector {
    dictionaries: Vec<(String, Dictionary)>,
}

impl DictionaryDetector {
    /// Creates a new detector from a configuration.
    pub fn new(config: &DictionaryRegistry) -> Result<Self, Box<dyn std::error::Error>> {
        let mut dictionaries = Vec::new();

        for (name, dict_config) in &config.dictionaries {
            let effective_mode = dict_config.effective_mode();
            let dictionary = match effective_mode {
                EncodingMode::ByteRange => {
                    let start = dict_config
                        .start_codepoint
                        .ok_or("ByteRange mode requires start_codepoint")?;
                    Dictionary::builder()
                        .mode(effective_mode)
                        .start_codepoint(start)
                        .build()?
                }
                _ => {
                    let chars: Vec<char> = dict_config.effective_chars()?.chars().collect();
                    let padding = dict_config.padding.as_ref().and_then(|s| s.chars().next());
                    let mut builder = Dictionary::builder().chars(chars).mode(effective_mode);
                    if let Some(p) = padding {
                        builder = builder.padding(p);
                    }
                    builder.build()?
                }
            };
            dictionaries.push((name.clone(), dictionary));
        }

        Ok(DictionaryDetector { dictionaries })
    }

    /// Detect which dictionary was likely used to encode the input.
    /// Returns matches sorted by confidence (highest first).
    pub fn detect(&self, input: &str) -> Vec<DictionaryMatch> {
        let input = input.trim();
        if input.is_empty() {
            return Vec::new();
        }

        let mut matches = Vec::new();

        for (name, dict) in &self.dictionaries {
            if let Some(confidence) = self.score_dictionary(input, dict) {
                matches.push(DictionaryMatch {
                    name: name.clone(),
                    confidence,
                    dictionary: dict.clone(),
                });
            }
        }

        // Sort by confidence descending
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        matches
    }

    /// Score how likely a dictionary matches the input.
    /// Returns Some(confidence) if it's a plausible match, None otherwise.
    fn score_dictionary(&self, input: &str, dict: &Dictionary) -> Option<f64> {
        let mut score = 0.0;
        let mut weight_sum = 0.0;

        // Weight for each scoring component
        const CHARSET_WEIGHT: f64 = 0.25;
        const SPECIFICITY_WEIGHT: f64 = 0.20; // Increased
        const PADDING_WEIGHT: f64 = 0.30; // Increased (very important for RFC standards)
        const LENGTH_WEIGHT: f64 = 0.15;
        const DECODE_WEIGHT: f64 = 0.10;

        // 1. Character set matching
        let charset_score = self.score_charset(input, dict);
        score += charset_score * CHARSET_WEIGHT;
        weight_sum += CHARSET_WEIGHT;

        // If character set score is too low, skip this dictionary
        if charset_score < 0.5 {
            return None;
        }

        // 1.5. Specificity - does this dictionary use a focused character set?
        let specificity_score = self.score_specificity(input, dict);
        score += specificity_score * SPECIFICITY_WEIGHT;
        weight_sum += SPECIFICITY_WEIGHT;

        // 2. Padding detection (for chunked modes)
        if let Some(padding_score) = self.score_padding(input, dict) {
            score += padding_score * PADDING_WEIGHT;
            weight_sum += PADDING_WEIGHT;
        }

        // 3. Length validation
        let length_score = self.score_length(input, dict);
        score += length_score * LENGTH_WEIGHT;
        weight_sum += LENGTH_WEIGHT;

        // 4. Decode validation (try to actually decode)
        if let Some(decode_score) = self.score_decode(input, dict) {
            score += decode_score * DECODE_WEIGHT;
            weight_sum += DECODE_WEIGHT;
        }

        // Normalize score
        if weight_sum > 0.0 {
            Some(score / weight_sum)
        } else {
            None
        }
    }

    /// Score based on character set matching.
    fn score_charset(&self, input: &str, dict: &Dictionary) -> f64 {
        // Get all unique characters in input (excluding whitespace and padding)
        let input_chars: HashSet<char> = input
            .chars()
            .filter(|c| !c.is_whitespace() && Some(*c) != dict.padding())
            .collect();

        if input_chars.is_empty() {
            return 0.0;
        }

        // For ByteRange mode, check if characters are in the expected range
        if let Some(start) = dict.start_codepoint() {
            let in_range = input_chars
                .iter()
                .filter(|&&c| {
                    let code = c as u32;
                    code >= start && code < start + 256
                })
                .count();
            return in_range as f64 / input_chars.len() as f64;
        }

        // Check if all input characters are in the dictionary
        let mut valid_count = 0;
        for c in &input_chars {
            if dict.decode_char(*c).is_some() {
                valid_count += 1;
            }
        }

        if valid_count < input_chars.len() {
            // Not all characters are valid - reject this dictionary
            return 0.0;
        }

        // All characters are valid. Now check how well the dictionary size matches
        let dict_size = dict.base();
        let input_unique = input_chars.len();

        // Calculate what percentage of the dictionary is actually used
        let usage_ratio = input_unique as f64 / dict_size as f64;

        // Prefer dictionaries where we use most of the character set
        // This helps distinguish base64 (64 chars) from base85 (85 chars)
        if usage_ratio > 0.7 {
            // We're using >70% of dictionary - excellent match
            1.0
        } else if usage_ratio > 0.5 {
            // We're using >50% of dictionary - good match
            0.85
        } else if usage_ratio > 0.3 {
            // We're using >30% of dictionary - okay match
            0.7
        } else {
            // We're using <30% of dictionary - probably wrong
            // (e.g., using 20 chars of a 85-char dictionary)
            0.5
        }
    }

    /// Score based on how specific/focused the dictionary character set is.
    /// Smaller, more focused dictionaries score higher.
    fn score_specificity(&self, _input: &str, dict: &Dictionary) -> f64 {
        let dict_size = dict.base();

        // Prefer smaller, more common dictionaries
        // This helps distinguish base64 (64) from base85 (85) when both match
        match dict_size {
            16 => 1.0,   // hex
            32 => 0.95,  // base32
            58 => 0.90,  // base58
            62 => 0.88,  // base62
            64 => 0.92,  // base64 (very common)
            85 => 0.70,  // base85 (less common)
            256 => 0.60, // base256
            _ if dict_size < 64 => 0.85,
            _ if dict_size < 128 => 0.75,
            _ => 0.65,
        }
    }

    /// Score based on padding character presence and position.
    fn score_padding(&self, input: &str, dict: &Dictionary) -> Option<f64> {
        let padding = dict.padding()?;

        // Chunked modes should have padding at the end (or no padding)
        if *dict.mode() == EncodingMode::Chunked {
            let has_padding = input.ends_with(padding);
            let padding_count = input.chars().filter(|c| *c == padding).count();

            if has_padding {
                // Padding should only be at the end
                let trimmed = input.trim_end_matches(padding);
                let internal_padding = trimmed.chars().any(|c| c == padding);

                if internal_padding {
                    Some(0.5) // Suspicious padding in middle
                } else if padding_count <= 3 {
                    Some(1.0) // Valid padding
                } else {
                    Some(0.3) // Too much padding
                }
            } else {
                // No padding is also valid for chunked mode
                Some(0.8)
            }
        } else {
            None
        }
    }

    /// Score based on input length validation for the encoding mode.
    fn score_length(&self, input: &str, dict: &Dictionary) -> f64 {
        let length = input.trim().len();

        match dict.mode() {
            EncodingMode::Chunked => {
                // Chunked mode should have specific alignment
                let base = dict.base();

                // Remove padding to check alignment
                let trimmed = if let Some(pad) = dict.padding() {
                    input.trim_end_matches(pad)
                } else {
                    input
                };

                // For base64 (6 bits per char), output should be multiple of 4
                // For base32 (5 bits per char), output should be multiple of 8
                // For base16 (4 bits per char), output should be multiple of 2
                let expected_multiple = match base {
                    64 => 4,
                    32 => 8,
                    16 => 2,
                    _ => return 0.5, // Unknown chunked base
                };

                if trimmed.len() % expected_multiple == 0 {
                    1.0
                } else {
                    0.3
                }
            }
            EncodingMode::ByteRange => {
                // ByteRange is 1:1 mapping, any length is valid
                1.0
            }
            EncodingMode::Radix => {
                // Radix conversion can produce any length
                if length > 0 { 1.0 } else { 0.0 }
            }
        }
    }

    /// Score based on whether the input can be successfully decoded.
    fn score_decode(&self, input: &str, dict: &Dictionary) -> Option<f64> {
        match decode(input, dict) {
            Ok(decoded) => {
                if decoded.is_empty() {
                    Some(0.5)
                } else {
                    // Successfully decoded!
                    Some(1.0)
                }
            }
            Err(_) => {
                // Failed to decode
                Some(0.0)
            }
        }
    }
}

/// Convenience function to detect dictionary from input.
pub fn detect_dictionary(input: &str) -> Result<Vec<DictionaryMatch>, Box<dyn std::error::Error>> {
    let config = DictionaryRegistry::load_with_overrides()?;
    let detector = DictionaryDetector::new(&config)?;
    Ok(detector.detect(input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode;

    #[test]
    fn test_detect_base64() {
        let config = DictionaryRegistry::load_default().unwrap();
        let detector = DictionaryDetector::new(&config).unwrap();

        // Standard base64 with padding
        let matches = detector.detect("SGVsbG8sIFdvcmxkIQ==");
        assert!(!matches.is_empty());
        // base64 and base64url are very similar, so either is acceptable
        assert!(matches[0].name == "base64" || matches[0].name == "base64url");
        assert!(matches[0].confidence > 0.7);
    }

    #[test]
    fn test_detect_base32() {
        let config = DictionaryRegistry::load_default().unwrap();
        let detector = DictionaryDetector::new(&config).unwrap();

        let matches = detector.detect("JBSWY3DPEBLW64TMMQ======");
        assert!(!matches.is_empty());
        // base32 should be in top 10 candidates (more dictionaries now)
        let base32_found = matches
            .iter()
            .take(10)
            .any(|m| m.name.starts_with("base32"));
        assert!(base32_found, "base32 should be in top 10 candidates");
    }

    #[test]
    fn test_detect_hex() {
        let config = DictionaryRegistry::load_default().unwrap();
        let detector = DictionaryDetector::new(&config).unwrap();

        let matches = detector.detect("48656c6c6f");
        assert!(!matches.is_empty());
        // hex or hex_radix are both correct
        assert!(matches[0].name == "hex" || matches[0].name == "hex_radix");
        assert!(matches[0].confidence > 0.8);
    }

    #[test]
    fn test_detect_from_encoded() {
        let config = DictionaryRegistry::load_default().unwrap();

        // Test with actual encoding
        let dict_config = config.get_dictionary("base64").unwrap();
        let chars: Vec<char> = dict_config.effective_chars().unwrap().chars().collect();
        let padding = dict_config.padding.as_ref().and_then(|s| s.chars().next());
        let mut builder = Dictionary::builder()
            .chars(chars)
            .mode(dict_config.effective_mode());
        if let Some(p) = padding {
            builder = builder.padding(p);
        }
        let dict = builder.build().unwrap();

        let data = b"Hello, World!";
        let encoded = encode(data, &dict);

        let detector = DictionaryDetector::new(&config).unwrap();
        let matches = detector.detect(&encoded);

        assert!(!matches.is_empty());
        // base64 and base64url only differ by 2 chars, so both are valid
        assert!(matches[0].name == "base64" || matches[0].name == "base64url");
    }

    #[test]
    fn test_detect_empty_input() {
        let config = DictionaryRegistry::load_default().unwrap();
        let detector = DictionaryDetector::new(&config).unwrap();

        let matches = detector.detect("");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_detect_invalid_input() {
        let config = DictionaryRegistry::load_default().unwrap();
        let detector = DictionaryDetector::new(&config).unwrap();

        // Input with characters not in any dictionary
        let matches = detector.detect("こんにちは世界");
        // Should return few or no high-confidence matches
        if !matches.is_empty() {
            assert!(matches[0].confidence < 0.5);
        }
    }
}
