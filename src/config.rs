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
    /// Works with any alphabet size. Output length varies with input.
    BaseConversion,
    /// Fixed-size bit chunking per RFC 4648.
    /// Requires power-of-two alphabet size. Supports padding.
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

/// Configuration for a single alphabet loaded from TOML.
#[derive(Debug, Deserialize, Clone)]
pub struct AlphabetConfig {
    /// The characters comprising the alphabet
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

/// Collection of alphabet configurations loaded from TOML files.
#[derive(Debug, Deserialize)]
pub struct AlphabetsConfig {
    /// Map of alphabet names to their configurations
    pub alphabets: HashMap<String, AlphabetConfig>,
}

impl AlphabetsConfig {
    /// Parses alphabet configurations from TOML content.
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }
    
    /// Loads the built-in alphabet configurations.
    ///
    /// Returns the default alphabets bundled with the library.
    pub fn load_default() -> Result<Self, Box<dyn std::error::Error>> {
        let content = include_str!("../alphabets.toml");
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
    /// 1. Built-in alphabets (from library)
    /// 2. `~/.config/base-d/alphabets.toml` (user overrides)
    /// 3. `./alphabets.toml` (project-local overrides)
    ///
    /// Later configurations override earlier ones for matching alphabet names.
    pub fn load_with_overrides() -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = Self::load_default()?;
        
        // Try to load user config from ~/.config/base-d/alphabets.toml
        if let Some(config_dir) = dirs::config_dir() {
            let user_config_path = config_dir.join("base-d").join("alphabets.toml");
            if user_config_path.exists() {
                match Self::load_from_file(&user_config_path) {
                    Ok(user_config) => {
                        config.merge(user_config);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load user config from {:?}: {}", user_config_path, e);
                    }
                }
            }
        }
        
        // Try to load local config from ./alphabets.toml
        let local_config_path = std::path::Path::new("alphabets.toml");
        if local_config_path.exists() {
            match Self::load_from_file(local_config_path) {
                Ok(local_config) => {
                    config.merge(local_config);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to load local config from {:?}: {}", local_config_path, e);
                }
            }
        }
        
        Ok(config)
    }
    
    /// Merges another configuration into this one.
    ///
    /// Alphabets from `other` override alphabets with the same name in `self`.
    pub fn merge(&mut self, other: AlphabetsConfig) {
        for (name, alphabet) in other.alphabets {
            self.alphabets.insert(name, alphabet);
        }
    }
    
    /// Retrieves an alphabet configuration by name.
    pub fn get_alphabet(&self, name: &str) -> Option<&AlphabetConfig> {
        self.alphabets.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_default_config() {
        let config = AlphabetsConfig::load_default().unwrap();
        assert!(config.alphabets.contains_key("cards"));
    }
    
    #[test]
    fn test_cards_alphabet_length() {
        let config = AlphabetsConfig::load_default().unwrap();
        let cards = config.get_alphabet("cards").unwrap();
        assert_eq!(cards.chars.chars().count(), 52);
    }
    
    #[test]
    fn test_base64_chunked_mode() {
        let config = AlphabetsConfig::load_default().unwrap();
        let base64 = config.get_alphabet("base64").unwrap();
        assert_eq!(base64.mode, EncodingMode::Chunked);
        assert_eq!(base64.padding, Some("=".to_string()));
    }
    
    #[test]
    fn test_base64_math_mode() {
        let config = AlphabetsConfig::load_default().unwrap();
        let base64_math = config.get_alphabet("base64_math").unwrap();
        assert_eq!(base64_math.mode, EncodingMode::BaseConversion);
    }
    
    #[test]
    fn test_merge_configs() {
        let mut config1 = AlphabetsConfig {
            alphabets: HashMap::new(),
        };
        config1.alphabets.insert("test1".to_string(), AlphabetConfig {
            chars: "ABC".to_string(),
            mode: EncodingMode::BaseConversion,
            padding: None,
            start_codepoint: None,
        });
        
        let mut config2 = AlphabetsConfig {
            alphabets: HashMap::new(),
        };
        config2.alphabets.insert("test2".to_string(), AlphabetConfig {
            chars: "XYZ".to_string(),
            mode: EncodingMode::BaseConversion,
            padding: None,
            start_codepoint: None,
        });
        config2.alphabets.insert("test1".to_string(), AlphabetConfig {
            chars: "DEF".to_string(),
            mode: EncodingMode::BaseConversion,
            padding: None,
            start_codepoint: None,
        });
        
        config1.merge(config2);
        
        assert_eq!(config1.alphabets.len(), 2);
        assert_eq!(config1.get_alphabet("test1").unwrap().chars, "DEF");
        assert_eq!(config1.get_alphabet("test2").unwrap().chars, "XYZ");
    }
    
    #[test]
    fn test_load_from_toml_string() {
        let toml_content = r#"
[alphabets.custom]
chars = "0123456789"
mode = "base_conversion"
"#;
        let config = AlphabetsConfig::from_toml(toml_content).unwrap();
        assert!(config.alphabets.contains_key("custom"));
        assert_eq!(config.get_alphabet("custom").unwrap().chars, "0123456789");
    }
}

