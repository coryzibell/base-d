use std::collections::HashMap;
use crate::config::EncodingMode;

#[derive(Debug, Clone)]
pub struct Alphabet {
    chars: Vec<char>,
    char_to_index: HashMap<char, usize>,
    mode: EncodingMode,
    padding: Option<char>,
    start_codepoint: Option<u32>,
}

impl Alphabet {
    pub fn new(chars: Vec<char>) -> Result<Self, String> {
        Self::new_with_mode(chars, EncodingMode::BaseConversion, None)
    }
    
    pub fn new_with_mode(chars: Vec<char>, mode: EncodingMode, padding: Option<char>) -> Result<Self, String> {
        Self::new_with_mode_and_range(chars, mode, padding, None)
    }
    
    pub fn new_with_mode_and_range(chars: Vec<char>, mode: EncodingMode, padding: Option<char>, start_codepoint: Option<u32>) -> Result<Self, String> {
        // ByteRange mode doesn't need chars, just validates start_codepoint
        if mode == EncodingMode::ByteRange {
            if let Some(start) = start_codepoint {
                // Validate that we can represent all 256 bytes
                if let Some(end_codepoint) = start.checked_add(255) {
                    if std::char::from_u32(end_codepoint).is_none() {
                        return Err(format!("Invalid Unicode range: {}-{}", start, end_codepoint));
                    }
                } else {
                    return Err("Start codepoint too high for 256-byte range".to_string());
                }
                
                return Ok(Alphabet {
                    chars: Vec::new(),
                    char_to_index: HashMap::new(),
                    mode,
                    padding,
                    start_codepoint: Some(start),
                });
            } else {
                return Err("ByteRange mode requires start_codepoint".to_string());
            }
        }
        
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
            start_codepoint: None,
        })
    }
    
    pub fn from_str(s: &str) -> Result<Self, String> {
        let chars: Vec<char> = s.chars().collect();
        Self::new(chars)
    }
    
    pub fn base(&self) -> usize {
        match self.mode {
            EncodingMode::ByteRange => 256,
            _ => self.chars.len(),
        }
    }
    
    pub fn mode(&self) -> &EncodingMode {
        &self.mode
    }
    
    pub fn padding(&self) -> Option<char> {
        self.padding
    }
    
    pub fn start_codepoint(&self) -> Option<u32> {
        self.start_codepoint
    }
    
    pub fn encode_digit(&self, digit: usize) -> Option<char> {
        match self.mode {
            EncodingMode::ByteRange => {
                if let Some(start) = self.start_codepoint {
                    if digit < 256 {
                        return std::char::from_u32(start + digit as u32);
                    }
                }
                None
            }
            _ => self.chars.get(digit).copied(),
        }
    }
    
    pub fn decode_char(&self, c: char) -> Option<usize> {
        match self.mode {
            EncodingMode::ByteRange => {
                if let Some(start) = self.start_codepoint {
                    let codepoint = c as u32;
                    if codepoint >= start && codepoint < start + 256 {
                        return Some((codepoint - start) as usize);
                    }
                }
                None
            }
            _ => self.char_to_index.get(&c).copied(),
        }
    }
}


