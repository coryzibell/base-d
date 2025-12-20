use serde::Deserialize;
use std::collections::HashMap;

// Include generated dictionary registry from build.rs
include!(concat!(env!("OUT_DIR"), "/registry.rs"));

/// Dictionary type: character-based or word-based.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryType {
    /// Character-based dictionary (traditional encoding)
    #[default]
    Char,
    /// Word-based dictionary (BIP-39, Diceware, etc.)
    Word,
}

/// Encoding strategy for converting binary data to text.
///
/// Different modes offer different tradeoffs between efficiency, compatibility,
/// and features.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum EncodingMode {
    /// True radix/base conversion treating data as a large number.
    /// Works with any dictionary size. Output length varies with input.
    /// Requires entire input before producing output (not streamable).
    #[default]
    #[serde(alias = "base_conversion")]
    Radix,
    /// Fixed-size bit chunking per RFC 4648.
    /// Requires power-of-two dictionary size. Supports padding.
    Chunked,
    /// Direct 1:1 byte-to-character mapping using Unicode codepoint ranges.
    /// Zero encoding overhead. Always 256 characters.
    ByteRange,
}

/// Configuration for a single dictionary loaded from TOML.
#[derive(Debug, Deserialize, Clone)]
pub struct DictionaryConfig {
    // === Type discriminant ===
    /// Dictionary type: "char" (default) or "word"
    #[serde(default, rename = "type")]
    pub dictionary_type: DictionaryType,

    // === Character-based fields ===
    /// The characters comprising the dictionary (explicit list)
    #[serde(default)]
    pub chars: String,
    /// Starting character for range-based dictionary definition
    /// Use with `length` to define sequential Unicode ranges
    #[serde(default)]
    pub start: Option<String>,
    /// Number of characters in range-based dictionary
    /// Use with `start` to define sequential Unicode ranges
    #[serde(default)]
    pub length: Option<usize>,
    /// Starting Unicode codepoint for ByteRange mode (256 chars)
    #[serde(default)]
    pub start_codepoint: Option<u32>,

    // === Word-based fields ===
    /// Inline word list for word-based dictionaries
    #[serde(default)]
    pub words: Option<Vec<String>>,
    /// Path to external word list file (one word per line)
    #[serde(default)]
    pub words_file: Option<String>,
    /// Delimiter between words in encoded output (default: " ")
    #[serde(default)]
    pub delimiter: Option<String>,
    /// Whether word matching is case-sensitive (default: false)
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    /// Names of sub-dictionaries for alternating word encoding (e.g., ["pgp_even", "pgp_odd"])
    #[serde(default)]
    pub alternating: Option<Vec<String>>,

    // === Common fields ===
    /// The encoding mode to use (auto-detected if not specified)
    #[serde(default)]
    pub mode: Option<EncodingMode>,
    /// Optional padding character (e.g., "=" for base64)
    #[serde(default)]
    pub padding: Option<String>,
    /// Whether this dictionary renders consistently across platforms (default: true)
    /// Dictionaries with common=false are excluded from random selection (--dejavu)
    #[serde(default = "default_true")]
    pub common: bool,
}

impl Default for DictionaryConfig {
    fn default() -> Self {
        Self {
            dictionary_type: DictionaryType::default(),
            chars: String::new(),
            start: None,
            length: None,
            start_codepoint: None,
            words: None,
            words_file: None,
            delimiter: None,
            case_sensitive: None,
            alternating: None,
            mode: None,
            padding: None,
            common: true, // default to common for random selection
        }
    }
}

impl DictionaryConfig {
    /// Returns the effective character set, generating from range if needed.
    ///
    /// Priority:
    /// 1. If `chars` is non-empty, use it directly
    /// 2. If `start` + `length` are set, generate sequential range
    /// 3. Otherwise return empty string (ByteRange mode uses start_codepoint instead)
    pub fn effective_chars(&self) -> Result<String, String> {
        // Explicit chars take priority
        if !self.chars.is_empty() {
            return Ok(self.chars.clone());
        }

        // Generate from start + length range
        if let (Some(start_str), Some(length)) = (&self.start, self.length) {
            let start_char = start_str
                .chars()
                .next()
                .ok_or("start must contain at least one character")?;
            let start_codepoint = start_char as u32;

            return Self::generate_range(start_codepoint, length);
        }

        // No chars defined - might be ByteRange mode
        Ok(String::new())
    }

    /// Generate a string of sequential Unicode characters from a range.
    fn generate_range(start: u32, length: usize) -> Result<String, String> {
        const MAX_UNICODE: u32 = 0x10FFFF;
        const SURROGATE_START: u32 = 0xD800;
        const SURROGATE_END: u32 = 0xDFFF;

        if length == 0 {
            return Err("length must be greater than 0".to_string());
        }

        let end = start
            .checked_add(length as u32 - 1)
            .ok_or("range exceeds maximum Unicode codepoint")?;

        if end > MAX_UNICODE {
            return Err(format!(
                "range end U+{:X} exceeds maximum Unicode codepoint U+{:X}",
                end, MAX_UNICODE
            ));
        }

        // Check for surrogate gap crossing
        let crosses_surrogates = start <= SURROGATE_END && end >= SURROGATE_START;
        if crosses_surrogates {
            return Err(format!(
                "range U+{:X}..U+{:X} crosses surrogate gap (U+D800..U+DFFF)",
                start, end
            ));
        }

        let mut result = String::with_capacity(length * 4); // UTF-8 worst case
        for i in 0..length {
            let codepoint = start + i as u32;
            match char::from_u32(codepoint) {
                Some(c) => result.push(c),
                None => return Err(format!("invalid codepoint U+{:X}", codepoint)),
            }
        }

        Ok(result)
    }

    /// Returns the effective encoding mode, auto-detecting if not explicitly set.
    ///
    /// Auto-detection rules:
    /// - ByteRange: Must be explicitly set (requires start_codepoint)
    /// - Chunked: If alphabet length is a power of 2
    /// - Radix: Otherwise (true base conversion)
    pub fn effective_mode(&self) -> EncodingMode {
        if let Some(mode) = &self.mode {
            return mode.clone();
        }

        // Auto-detect based on alphabet length
        let len = if self.start_codepoint.is_some() {
            // ByteRange must be explicit, but if someone sets start_codepoint
            // without mode, assume they want ByteRange
            return EncodingMode::ByteRange;
        } else if let Some(length) = self.length {
            // Range-based definition
            length
        } else {
            self.chars.chars().count()
        };

        if len > 0 && len.is_power_of_two() {
            EncodingMode::Chunked
        } else {
            EncodingMode::Radix
        }
    }
}

fn default_true() -> bool {
    true
}

/// Collection of dictionary configurations loaded from TOML files.
#[derive(Debug, Deserialize)]
pub struct DictionaryRegistry {
    /// Map of dictionary names to their configurations
    pub dictionaries: HashMap<String, DictionaryConfig>,
    /// Compression algorithm configurations
    #[serde(default)]
    pub compression: HashMap<String, CompressionConfig>,
    /// Global settings
    #[serde(default)]
    pub settings: Settings,
}

/// Configuration for a compression algorithm.
#[derive(Debug, Deserialize, Clone)]
pub struct CompressionConfig {
    /// Default compression level
    pub default_level: u32,
}

/// xxHash-specific settings.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct XxHashSettings {
    /// Default seed for xxHash algorithms
    #[serde(default)]
    pub default_seed: u64,
    /// Path to default secret file for XXH3 variants
    #[serde(default)]
    pub default_secret_file: Option<String>,
}

/// Global settings for base-d.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Settings {
    /// Default dictionary - if not set, requires explicit -e or --dejavu
    #[serde(default)]
    pub default_dictionary: Option<String>,
    /// xxHash configuration
    #[serde(default)]
    pub xxhash: XxHashSettings,
}

impl DictionaryRegistry {
    /// Parses dictionary configurations from TOML content.
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    /// Loads the built-in dictionary configurations.
    ///
    /// Returns the default dictionaries bundled with the library.
    pub fn load_default() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            dictionaries: build_registry(),
            compression: HashMap::new(),
            settings: Settings::default(),
        })
    }

    /// Loads configuration from a custom file path.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_toml(&content)?)
    }

    /// Loads configuration with user overrides from standard locations.
    ///
    /// Searches in priority order:
    /// 1. Built-in dictionaries (from library)
    /// 2. `~/.config/base-d/dictionaries.toml` (user overrides)
    /// 3. `./dictionaries.toml` (project-local overrides)
    ///
    /// Later configurations override earlier ones for matching dictionary names.
    pub fn load_with_overrides() -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = Self::load_default()?;

        // Try to load user config from ~/.config/base-d/dictionaries.toml
        if let Some(config_dir) = dirs::config_dir() {
            let user_config_path = config_dir.join("base-d").join("dictionaries.toml");
            if user_config_path.exists() {
                match Self::load_from_file(&user_config_path) {
                    Ok(user_config) => {
                        config.merge(user_config);
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to load user config from {:?}: {}",
                            user_config_path, e
                        );
                    }
                }
            }
        }

        // Try to load local config from ./dictionaries.toml
        let local_config_path = std::path::Path::new("dictionaries.toml");
        if local_config_path.exists() {
            match Self::load_from_file(local_config_path) {
                Ok(local_config) => {
                    config.merge(local_config);
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to load local config from {:?}: {}",
                        local_config_path, e
                    );
                }
            }
        }

        Ok(config)
    }

    /// Merges another configuration into this one.
    ///
    /// Dictionaries from `other` override dictionaries with the same name in `self`.
    pub fn merge(&mut self, other: DictionaryRegistry) {
        for (name, dictionary) in other.dictionaries {
            self.dictionaries.insert(name, dictionary);
        }
    }

    /// Retrieves an dictionary configuration by name.
    pub fn get_dictionary(&self, name: &str) -> Option<&DictionaryConfig> {
        self.dictionaries.get(name)
    }

    /// Builds a ready-to-use Dictionary from a named configuration.
    ///
    /// This is a convenience method that handles the common pattern of:
    /// 1. Looking up the dictionary config
    /// 2. Getting effective chars
    /// 3. Building the Dictionary with proper mode/padding
    ///
    /// # Example
    /// ```
    /// # use base_d::DictionaryRegistry;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let registry = DictionaryRegistry::load_default()?;
    /// let dict = registry.dictionary("base64")?;
    /// let encoded = base_d::encode(b"Hello", &dict);
    /// # Ok(())
    /// # }
    /// ```
    pub fn dictionary(
        &self,
        name: &str,
    ) -> Result<crate::Dictionary, crate::encoders::algorithms::errors::DictionaryNotFoundError>
    {
        let config = self.get_dictionary(name).ok_or_else(|| {
            crate::encoders::algorithms::errors::DictionaryNotFoundError::new(name)
        })?;

        self.build_dictionary(config).map_err(|e| {
            crate::encoders::algorithms::errors::DictionaryNotFoundError::with_cause(name, e)
        })
    }

    /// Returns a random dictionary suitable for encoding.
    ///
    /// Only selects from dictionaries marked as `common = true` (the default).
    /// These are dictionaries that render consistently across platforms.
    ///
    /// # Example
    /// ```
    /// # use base_d::DictionaryRegistry;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let registry = DictionaryRegistry::load_default()?;
    /// let (name, dict) = registry.random()?;
    /// let encoded = base_d::encode(b"Hello", &dict);
    /// # Ok(())
    /// # }
    /// ```
    pub fn random(&self) -> Result<(String, crate::Dictionary), Box<dyn std::error::Error>> {
        use rand::seq::IteratorRandom;

        let common_names: Vec<&String> = self
            .dictionaries
            .iter()
            .filter(|(_, config)| {
                // Only include common, character-based dictionaries
                config.common && config.dictionary_type == DictionaryType::Char
            })
            .map(|(name, _)| name)
            .collect();

        let name = common_names
            .into_iter()
            .choose(&mut rand::rng())
            .ok_or("No common dictionaries available")?;

        let dict = self.dictionary(name)?;
        Ok((name.clone(), dict))
    }

    /// Returns a list of all dictionary names.
    pub fn names(&self) -> Vec<&str> {
        self.dictionaries.keys().map(|s| s.as_str()).collect()
    }

    /// Returns a list of common dictionary names (suitable for random selection).
    pub fn common_names(&self) -> Vec<&str> {
        self.dictionaries
            .iter()
            .filter(|(_, config)| config.common)
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Internal helper to build a Dictionary from a DictionaryConfig.
    fn build_dictionary(&self, config: &DictionaryConfig) -> Result<crate::Dictionary, String> {
        use crate::core::config::EncodingMode;

        let mode = config.effective_mode();

        // ByteRange mode uses start_codepoint, not chars
        if mode == EncodingMode::ByteRange {
            let start = config
                .start_codepoint
                .ok_or("ByteRange mode requires start_codepoint")?;
            return crate::Dictionary::builder()
                .mode(mode)
                .start_codepoint(start)
                .build();
        }

        // Get effective chars (handles both explicit and range-based)
        let chars_str = config.effective_chars()?;
        let chars: Vec<char> = chars_str.chars().collect();

        // Build with optional padding
        let mut builder = crate::Dictionary::builder().chars(chars).mode(mode);

        if let Some(pad_str) = &config.padding
            && let Some(pad_char) = pad_str.chars().next()
        {
            builder = builder.padding(pad_char);
        }

        builder.build()
    }

    /// Builds a WordDictionary from a named configuration.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Dictionary not found
    /// - Dictionary is not word-type
    /// - Word list file cannot be read
    /// - Word dictionary building fails
    ///
    /// # Example
    /// ```
    /// # use base_d::DictionaryRegistry;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let registry = DictionaryRegistry::load_default()?;
    /// // Would work if bip39 is defined as a word dictionary
    /// // let dict = registry.word_dictionary("bip39")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn word_dictionary(
        &self,
        name: &str,
    ) -> Result<crate::WordDictionary, crate::encoders::algorithms::errors::DictionaryNotFoundError>
    {
        let config = self.get_dictionary(name).ok_or_else(|| {
            crate::encoders::algorithms::errors::DictionaryNotFoundError::new(name)
        })?;

        // Verify it's a word dictionary
        if config.dictionary_type != DictionaryType::Word {
            return Err(
                crate::encoders::algorithms::errors::DictionaryNotFoundError::with_cause(
                    name,
                    format!(
                        "Dictionary '{}' is not a word dictionary (type is {:?})",
                        name, config.dictionary_type
                    ),
                ),
            );
        }

        self.build_word_dictionary(config).map_err(|e| {
            crate::encoders::algorithms::errors::DictionaryNotFoundError::with_cause(name, e)
        })
    }

    /// Internal helper to build a WordDictionary from a DictionaryConfig.
    fn build_word_dictionary(
        &self,
        config: &DictionaryConfig,
    ) -> Result<crate::WordDictionary, String> {
        let mut builder = crate::WordDictionary::builder();

        // Get words from inline list, file, or builtin
        if let Some(ref words) = config.words {
            builder = builder.words(words.clone());
        } else if let Some(ref words_file) = config.words_file {
            // Check for embedded word lists first (generated by build.rs)
            let content = if let Some(embedded) = get_embedded_wordlist(words_file) {
                embedded.to_string()
            } else {
                // Check for builtin word lists
                match words_file.as_str() {
                    "builtin:bip39" | "builtin:bip39-english" => {
                        crate::wordlists::BIP39_ENGLISH.to_string()
                    }
                    "builtin:eff_long" | "builtin:eff-long" => {
                        crate::wordlists::EFF_LONG.to_string()
                    }
                    "builtin:eff_short1" | "builtin:eff-short1" => {
                        crate::wordlists::EFF_SHORT1.to_string()
                    }
                    "builtin:eff_short2" | "builtin:eff-short2" => {
                        crate::wordlists::EFF_SHORT2.to_string()
                    }
                    "builtin:diceware" => crate::wordlists::DICEWARE.to_string(),
                    "builtin:pgp_even" | "builtin:pgp-even" => {
                        crate::wordlists::PGP_EVEN.to_string()
                    }
                    "builtin:pgp_odd" | "builtin:pgp-odd" => crate::wordlists::PGP_ODD.to_string(),
                    "builtin:nato" => crate::wordlists::NATO.to_string(),
                    "builtin:buzzwords" => crate::wordlists::BUZZWORDS.to_string(),
                    "builtin:klingon" => crate::wordlists::KLINGON.to_string(),
                    "builtin:pokemon" => crate::wordlists::POKEMON.to_string(),
                    _ => {
                        // Resolve path (support ~ expansion)
                        let expanded = shellexpand::tilde(words_file);
                        std::fs::read_to_string(expanded.as_ref()).map_err(|e| {
                            format!("Failed to read words file '{}': {}", words_file, e)
                        })?
                    }
                }
            };
            builder = builder.words_from_str(&content);
        } else {
            return Err("Word dictionary must have 'words' or 'words_file'".to_string());
        }

        // Set optional delimiter
        if let Some(ref delimiter) = config.delimiter {
            builder = builder.delimiter(delimiter.clone());
        }

        // Set case sensitivity
        if let Some(case_sensitive) = config.case_sensitive {
            builder = builder.case_sensitive(case_sensitive);
        }

        builder.build()
    }

    /// Builds an AlternatingWordDictionary from a named configuration.
    ///
    /// This is used for PGP-style biometric word lists where even/odd bytes
    /// use different dictionaries.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Dictionary not found
    /// - Dictionary is not word-type
    /// - Dictionary does not have alternating field set
    /// - Any of the sub-dictionaries cannot be loaded
    ///
    /// # Example
    /// ```ignore
    /// # use base_d::DictionaryRegistry;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let registry = DictionaryRegistry::load_default()?;
    /// let dict = registry.alternating_word_dictionary("pgp")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn alternating_word_dictionary(
        &self,
        name: &str,
    ) -> Result<crate::AlternatingWordDictionary, crate::encoders::algorithms::errors::DictionaryNotFoundError>
    {
        let config = self.get_dictionary(name).ok_or_else(|| {
            crate::encoders::algorithms::errors::DictionaryNotFoundError::new(name)
        })?;

        // Verify it's a word dictionary
        if config.dictionary_type != DictionaryType::Word {
            return Err(
                crate::encoders::algorithms::errors::DictionaryNotFoundError::with_cause(
                    name,
                    format!(
                        "Dictionary '{}' is not a word dictionary (type is {:?})",
                        name, config.dictionary_type
                    ),
                ),
            );
        }

        // Verify it has alternating field
        let alternating_names = config.alternating.as_ref().ok_or_else(|| {
            crate::encoders::algorithms::errors::DictionaryNotFoundError::with_cause(
                name,
                format!(
                    "Dictionary '{}' is not an alternating dictionary (missing 'alternating' field)",
                    name
                ),
            )
        })?;

        self.build_alternating_word_dictionary(config, alternating_names)
            .map_err(|e| {
                crate::encoders::algorithms::errors::DictionaryNotFoundError::with_cause(name, e)
            })
    }

    /// Internal helper to build an AlternatingWordDictionary from a DictionaryConfig.
    fn build_alternating_word_dictionary(
        &self,
        config: &DictionaryConfig,
        alternating_names: &[String],
    ) -> Result<crate::AlternatingWordDictionary, String> {
        if alternating_names.is_empty() {
            return Err("Alternating dictionary must have at least one sub-dictionary".to_string());
        }

        // Load all sub-dictionaries
        let mut dictionaries = Vec::with_capacity(alternating_names.len());
        for dict_name in alternating_names {
            let sub_dict = self
                .word_dictionary(dict_name)
                .map_err(|e| format!("Failed to load sub-dictionary '{}': {}", dict_name, e))?;
            dictionaries.push(sub_dict);
        }

        // Get delimiter and case sensitivity from parent config
        let delimiter = config
            .delimiter
            .clone()
            .unwrap_or_else(|| " ".to_string());
        let case_sensitive = config.case_sensitive.unwrap_or(false);

        Ok(crate::AlternatingWordDictionary::new(
            dictionaries,
            delimiter,
            case_sensitive,
        ))
    }

    /// Returns the dictionary type for a named dictionary.
    ///
    /// Returns `None` if the dictionary is not found.
    pub fn dictionary_type(&self, name: &str) -> Option<DictionaryType> {
        self.get_dictionary(name).map(|c| c.dictionary_type.clone())
    }

    /// Checks if a dictionary is word-based.
    pub fn is_word_dictionary(&self, name: &str) -> bool {
        self.dictionary_type(name) == Some(DictionaryType::Word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_config() {
        let config = DictionaryRegistry::load_default().unwrap();
        assert!(config.dictionaries.contains_key("cards"));
    }

    #[test]
    fn test_cards_dictionary_length() {
        let config = DictionaryRegistry::load_default().unwrap();
        let cards = config.get_dictionary("cards").unwrap();
        assert_eq!(cards.chars.chars().count(), 52);
    }

    #[test]
    fn test_base64_chunked_mode() {
        let config = DictionaryRegistry::load_default().unwrap();
        let base64 = config.get_dictionary("base64").unwrap();
        assert_eq!(base64.effective_mode(), EncodingMode::Chunked);
        assert_eq!(base64.padding, Some("=".to_string()));
    }

    #[test]
    fn test_base64_radix_mode() {
        let config = DictionaryRegistry::load_default().unwrap();
        let base64_radix = config.get_dictionary("base64_radix").unwrap();
        assert_eq!(base64_radix.effective_mode(), EncodingMode::Radix);
    }

    #[test]
    fn test_auto_detection_power_of_two() {
        // Power of 2 → Chunked
        let config = DictionaryConfig {
            chars: "ABCD".to_string(), // 4 = 2^2
            ..Default::default()
        };
        assert_eq!(config.effective_mode(), EncodingMode::Chunked);

        // Not power of 2 → Radix
        let config = DictionaryConfig {
            chars: "ABC".to_string(), // 3 ≠ 2^n
            ..Default::default()
        };
        assert_eq!(config.effective_mode(), EncodingMode::Radix);
    }

    #[test]
    fn test_explicit_mode_override() {
        // Explicit mode overrides auto-detection
        let config = DictionaryConfig {
            chars: "ABCD".to_string(),       // Would be Chunked
            mode: Some(EncodingMode::Radix), // But explicitly set to Radix
            ..Default::default()
        };
        assert_eq!(config.effective_mode(), EncodingMode::Radix);
    }

    #[test]
    fn test_merge_configs() {
        let mut config1 = DictionaryRegistry {
            dictionaries: HashMap::new(),
            compression: HashMap::new(),
            settings: Settings::default(),
        };
        config1.dictionaries.insert(
            "test1".to_string(),
            DictionaryConfig {
                chars: "ABC".to_string(),
                mode: Some(EncodingMode::Radix),
                ..Default::default()
            },
        );

        let mut config2 = DictionaryRegistry {
            dictionaries: HashMap::new(),
            compression: HashMap::new(),
            settings: Settings::default(),
        };
        config2.dictionaries.insert(
            "test2".to_string(),
            DictionaryConfig {
                chars: "XYZ".to_string(),
                mode: Some(EncodingMode::Radix),
                ..Default::default()
            },
        );
        config2.dictionaries.insert(
            "test1".to_string(),
            DictionaryConfig {
                chars: "DEF".to_string(),
                mode: Some(EncodingMode::Radix),
                ..Default::default()
            },
        );

        config1.merge(config2);

        assert_eq!(config1.dictionaries.len(), 2);
        assert_eq!(config1.get_dictionary("test1").unwrap().chars, "DEF");
        assert_eq!(config1.get_dictionary("test2").unwrap().chars, "XYZ");
    }

    #[test]
    fn test_load_from_toml_string() {
        let toml_content = r#"
[dictionaries.custom]
chars = "0123456789"
mode = "base_conversion"
"#;
        let config = DictionaryRegistry::from_toml(toml_content).unwrap();
        assert!(config.dictionaries.contains_key("custom"));
        assert_eq!(config.get_dictionary("custom").unwrap().chars, "0123456789");
    }

    #[test]
    fn test_effective_chars_from_explicit() {
        let config = DictionaryConfig {
            chars: "ABCD".to_string(),
            ..Default::default()
        };
        assert_eq!(config.effective_chars().unwrap(), "ABCD");
    }

    #[test]
    fn test_effective_chars_from_range() {
        let config = DictionaryConfig {
            start: Some("A".to_string()),
            length: Some(4),
            ..Default::default()
        };
        assert_eq!(config.effective_chars().unwrap(), "ABCD");
    }

    #[test]
    fn test_effective_chars_explicit_takes_priority() {
        // Explicit chars should override start+length
        let config = DictionaryConfig {
            chars: "XYZ".to_string(),
            start: Some("A".to_string()),
            length: Some(4),
            ..Default::default()
        };
        assert_eq!(config.effective_chars().unwrap(), "XYZ");
    }

    #[test]
    fn test_effective_chars_unicode_range() {
        // Test generating a range starting from a Unicode character
        let config = DictionaryConfig {
            start: Some("가".to_string()), // Korean Hangul U+AC00
            length: Some(4),
            ..Default::default()
        };
        let result = config.effective_chars().unwrap();
        assert_eq!(result.chars().count(), 4);
        assert_eq!(result, "가각갂갃");
    }

    #[test]
    fn test_effective_chars_surrogate_gap_error() {
        // Range crossing surrogate gap should error
        let config = DictionaryConfig {
            start: Some("\u{D700}".to_string()), // Just before surrogates
            length: Some(512),                   // Would cross into surrogate range
            ..Default::default()
        };
        assert!(config.effective_chars().is_err());
    }

    #[test]
    fn test_effective_chars_exceeds_unicode_max() {
        // Range exceeding max Unicode should error
        let config = DictionaryConfig {
            start: Some("\u{10FFFE}".to_string()), // Near end of Unicode
            length: Some(10),                      // Would exceed U+10FFFF
            ..Default::default()
        };
        assert!(config.effective_chars().is_err());
    }

    #[test]
    fn test_effective_mode_with_length_field() {
        // Auto-detect should use length field when chars is empty
        let config = DictionaryConfig {
            start: Some("A".to_string()),
            length: Some(64), // 64 = 2^6 → Chunked
            ..Default::default()
        };
        assert_eq!(config.effective_mode(), EncodingMode::Chunked);

        let config = DictionaryConfig {
            start: Some("A".to_string()),
            length: Some(52), // 52 ≠ 2^n → Radix
            ..Default::default()
        };
        assert_eq!(config.effective_mode(), EncodingMode::Radix);
    }
}
