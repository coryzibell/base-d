use serde::Deserialize;
use std::collections::HashMap;

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
    /// The encoding mode to use (auto-detected if not specified)
    #[serde(default)]
    pub mode: Option<EncodingMode>,
    /// Optional padding character (e.g., "=" for base64)
    #[serde(default)]
    pub padding: Option<String>,
    /// Starting Unicode codepoint for ByteRange mode (256 chars)
    #[serde(default)]
    pub start_codepoint: Option<u32>,
    /// Whether this dictionary renders consistently across platforms (default: true)
    /// Dictionaries with common=false are excluded from random selection (--dejavu)
    #[serde(default = "default_true")]
    pub common: bool,
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
            let start_char = start_str.chars().next()
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

        let end = start.checked_add(length as u32 - 1)
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
        let content = include_str!("../../dictionaries.toml");
        Ok(Self::from_toml(content)?)
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
            mode: None,
            padding: None,
            start_codepoint: None,
            start: None,
            length: None,
            common: true,
        };
        assert_eq!(config.effective_mode(), EncodingMode::Chunked);

        // Not power of 2 → Radix
        let config = DictionaryConfig {
            chars: "ABC".to_string(), // 3 ≠ 2^n
            mode: None,
            padding: None,
            start_codepoint: None,
            start: None,
            length: None,
            common: true,
        };
        assert_eq!(config.effective_mode(), EncodingMode::Radix);
    }

    #[test]
    fn test_explicit_mode_override() {
        // Explicit mode overrides auto-detection
        let config = DictionaryConfig {
            chars: "ABCD".to_string(), // Would be Chunked
            mode: Some(EncodingMode::Radix), // But explicitly set to Radix
            padding: None,
            start_codepoint: None,
            start: None,
            length: None,
            common: true,
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
                padding: None,
                start_codepoint: None,
                start: None,
                length: None,
                common: true,
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
                padding: None,
                start_codepoint: None,
                start: None,
                length: None,
                common: true,
            },
        );
        config2.dictionaries.insert(
            "test1".to_string(),
            DictionaryConfig {
                chars: "DEF".to_string(),
                mode: Some(EncodingMode::Radix),
                padding: None,
                start_codepoint: None,
                start: None,
                length: None,
                common: true,
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
            mode: None,
            padding: None,
            start_codepoint: None,
            start: None,
            length: None,
            common: true,
        };
        assert_eq!(config.effective_chars().unwrap(), "ABCD");
    }

    #[test]
    fn test_effective_chars_from_range() {
        let config = DictionaryConfig {
            chars: String::new(),
            mode: None,
            padding: None,
            start_codepoint: None,
            start: Some("A".to_string()),
            length: Some(4),
            common: true,
        };
        assert_eq!(config.effective_chars().unwrap(), "ABCD");
    }

    #[test]
    fn test_effective_chars_explicit_takes_priority() {
        // Explicit chars should override start+length
        let config = DictionaryConfig {
            chars: "XYZ".to_string(),
            mode: None,
            padding: None,
            start_codepoint: None,
            start: Some("A".to_string()),
            length: Some(4),
            common: true,
        };
        assert_eq!(config.effective_chars().unwrap(), "XYZ");
    }

    #[test]
    fn test_effective_chars_unicode_range() {
        // Test generating a range starting from a Unicode character
        let config = DictionaryConfig {
            chars: String::new(),
            mode: None,
            padding: None,
            start_codepoint: None,
            start: Some("가".to_string()), // Korean Hangul U+AC00
            length: Some(4),
            common: true,
        };
        let result = config.effective_chars().unwrap();
        assert_eq!(result.chars().count(), 4);
        assert_eq!(result, "가각갂갃");
    }

    #[test]
    fn test_effective_chars_surrogate_gap_error() {
        // Range crossing surrogate gap should error
        let config = DictionaryConfig {
            chars: String::new(),
            mode: None,
            padding: None,
            start_codepoint: None,
            start: Some("\u{D700}".to_string()), // Just before surrogates
            length: Some(512), // Would cross into surrogate range
            common: true,
        };
        assert!(config.effective_chars().is_err());
    }

    #[test]
    fn test_effective_chars_exceeds_unicode_max() {
        // Range exceeding max Unicode should error
        let config = DictionaryConfig {
            chars: String::new(),
            mode: None,
            padding: None,
            start_codepoint: None,
            start: Some("\u{10FFFE}".to_string()), // Near end of Unicode
            length: Some(10), // Would exceed U+10FFFF
            common: true,
        };
        assert!(config.effective_chars().is_err());
    }

    #[test]
    fn test_effective_mode_with_length_field() {
        // Auto-detect should use length field when chars is empty
        let config = DictionaryConfig {
            chars: String::new(),
            mode: None,
            padding: None,
            start_codepoint: None,
            start: Some("A".to_string()),
            length: Some(64), // 64 = 2^6 → Chunked
            common: true,
        };
        assert_eq!(config.effective_mode(), EncodingMode::Chunked);

        let config = DictionaryConfig {
            chars: String::new(),
            mode: None,
            padding: None,
            start_codepoint: None,
            start: Some("A".to_string()),
            length: Some(52), // 52 ≠ 2^n → Radix
            common: true,
        };
        assert_eq!(config.effective_mode(), EncodingMode::Radix);
    }
}
