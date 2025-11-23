use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EncodingMode {
    BaseConversion,
    Chunked,
}

impl Default for EncodingMode {
    fn default() -> Self {
        EncodingMode::BaseConversion
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct AlphabetConfig {
    pub chars: String,
    #[serde(default)]
    pub mode: EncodingMode,
    #[serde(default)]
    pub padding: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AlphabetsConfig {
    pub alphabets: HashMap<String, AlphabetConfig>,
}

impl AlphabetsConfig {
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }
    
    pub fn load_default() -> Result<Self, Box<dyn std::error::Error>> {
        let content = include_str!("../alphabets.toml");
        Ok(Self::from_toml(content)?)
    }
    
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
}

