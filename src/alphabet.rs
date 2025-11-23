use std::collections::HashMap;
use crate::config::EncodingMode;

#[derive(Debug, Clone)]
pub struct Alphabet {
    chars: Vec<char>,
    char_to_index: HashMap<char, usize>,
    mode: EncodingMode,
    padding: Option<char>,
}

impl Alphabet {
    pub fn new(chars: Vec<char>) -> Result<Self, String> {
        Self::new_with_mode(chars, EncodingMode::BaseConversion, None)
    }
    
    pub fn new_with_mode(chars: Vec<char>, mode: EncodingMode, padding: Option<char>) -> Result<Self, String> {
        if chars.is_empty() {
            return Err("Alphabet cannot be empty".to_string());
        }
        
        // Validate alphabet size for chunked mode
        if mode == EncodingMode::Chunked {
            let base = chars.len();
            if !base.is_power_of_two() {
                return Err(format!("Chunked mode requires power-of-two alphabet size, got {}", base));
            }
        }
        
        let mut char_to_index = HashMap::new();
        for (i, &c) in chars.iter().enumerate() {
            if char_to_index.insert(c, i).is_some() {
                return Err(format!("Duplicate character in alphabet: {}", c));
            }
        }
        
        Ok(Alphabet {
            chars,
            char_to_index,
            mode,
            padding,
        })
    }
    
    pub fn from_str(s: &str) -> Result<Self, String> {
        let chars: Vec<char> = s.chars().collect();
        Self::new(chars)
    }
    
    pub fn base(&self) -> usize {
        self.chars.len()
    }
    
    pub fn mode(&self) -> &EncodingMode {
        &self.mode
    }
    
    pub fn padding(&self) -> Option<char> {
        self.padding
    }
    
    pub fn encode_digit(&self, digit: usize) -> Option<char> {
        self.chars.get(digit).copied()
    }
    
    pub fn decode_char(&self, c: char) -> Option<usize> {
        self.char_to_index.get(&c).copied()
    }
}


