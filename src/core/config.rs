use serde::Deserialize;
use std::collections::HashMap;

/// Encoding strategy for converting binary data to text.
///
/// Different modes offer different tradeoffs between efficiency, compatibility,
/// and features.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EncodingMode {
    /// Mathematical base conversion treating data as a large number.
    /// Works with any dictionary size. Output length varies with input.
    BaseConversion,
    /// Fixed-size bit chunking per RFC 4648.
    /// Requires power-of-two dictionary size. Supports padding.
    Chunked,
    /// Direct 1:1 byte-to-character mapping using Unicode codepoint ranges.
    /// Zero encoding overhead. Always 256 characters.
    ByteRange,
}

impl Default for EncodingMode {
    fn default() -> Self {
        EncodingMode::BaseConversion
    }
}

/// Configuration for a single dictionary loaded from TOML.
#[derive(Debug, Deserialize, Clone)]
pub struct DictionaryConfig {
    /// The characters comprising the dictionary
    #[serde(default)]
    pub chars: String,
    /// The encoding mode to use
    #[serde(default)]
    pub mode: EncodingMode,
    /// Optional padding character (e.g., "=" for base64)
    #[serde(default)]
    pub padding: Option<String>,
    /// Starting Unicode codepoint for ByteRange mode
    #[serde(default)]
    pub start_codepoint: Option<u32>,
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
    /// Returns the default alphabets bundled with the library.
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
    /// Alphabets from `other` override alphabets with the same name in `self`.
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
    fn test_cards_alphabet_length() {
        let config = DictionaryRegistry::load_default().unwrap();
        let cards = config.get_dictionary("cards").unwrap();
        assert_eq!(cards.chars.chars().count(), 52);
    }

    #[test]
    fn test_base64_chunked_mode() {
        let config = DictionaryRegistry::load_default().unwrap();
        let base64 = config.get_dictionary("base64").unwrap();
        assert_eq!(base64.mode, EncodingMode::Chunked);
        assert_eq!(base64.padding, Some("=".to_string()));
    }

    #[test]
    fn test_base64_math_mode() {
        let config = DictionaryRegistry::load_default().unwrap();
        let base64_math = config.get_dictionary("base64_math").unwrap();
        assert_eq!(base64_math.mode, EncodingMode::BaseConversion);
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
                mode: EncodingMode::BaseConversion,
                padding: None,
                start_codepoint: None,
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
                mode: EncodingMode::BaseConversion,
                padding: None,
                start_codepoint: None,
            },
        );
        config2.dictionaries.insert(
            "test1".to_string(),
            DictionaryConfig {
                chars: "DEF".to_string(),
                mode: EncodingMode::BaseConversion,
                padding: None,
                start_codepoint: None,
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
}
