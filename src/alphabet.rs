use std::collections::HashMap;
use crate::config::EncodingMode;

/// Represents an encoding alphabet with its characters and configuration.
///
/// An alphabet defines the character set and encoding mode used for converting
/// binary data to text. Supports three modes: mathematical base conversion,
/// chunked (RFC 4648), and byte-range mapping.
#[derive(Debug, Clone)]
pub struct Alphabet {
    chars: Vec<char>,
    char_to_index: HashMap<char, usize>,
    mode: EncodingMode,
    padding: Option<char>,
    start_codepoint: Option<u32>,
}

impl Alphabet {
    /// Creates a new alphabet with default settings (BaseConversion mode, no padding).
    ///
    /// # Arguments
    ///
    /// * `chars` - Vector of characters to use in the alphabet
    ///
    /// # Errors
    ///
    /// Returns an error if the alphabet is empty or contains duplicate characters.
    pub fn new(chars: Vec<char>) -> Result<Self, String> {
        Self::new_with_mode(chars, EncodingMode::BaseConversion, None)
    }
    
    /// Creates a new alphabet with specified encoding mode and optional padding.
    ///
    /// # Arguments
    ///
    /// * `chars` - Vector of characters to use in the alphabet
    /// * `mode` - Encoding mode (BaseConversion, Chunked, or ByteRange)
    /// * `padding` - Optional padding character (typically '=' for RFC modes)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The alphabet is empty or contains duplicates
    /// - Chunked mode is used with a non-power-of-two alphabet size
    pub fn new_with_mode(chars: Vec<char>, mode: EncodingMode, padding: Option<char>) -> Result<Self, String> {
        Self::new_with_mode_and_range(chars, mode, padding, None)
    }
    
    /// Creates a new alphabet with full configuration including byte-range support.
    ///
    /// # Arguments
    ///
    /// * `chars` - Vector of characters (empty for ByteRange mode)
    /// * `mode` - Encoding mode
    /// * `padding` - Optional padding character
    /// * `start_codepoint` - Starting Unicode codepoint for ByteRange mode
    ///
    /// # Errors
    ///
    /// Returns an error if configuration is invalid for the specified mode.
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
    
    /// Creates an alphabet from a string of characters.
    ///
    /// # Arguments
    ///
    /// * `s` - String containing the alphabet characters
    pub fn from_str(s: &str) -> Result<Self, String> {
        let chars: Vec<char> = s.chars().collect();
        Self::new(chars)
    }
    
    /// Returns the base (radix) of the alphabet.
    ///
    /// For ByteRange mode, always returns 256. Otherwise returns the number of characters.
    pub fn base(&self) -> usize {
        match self.mode {
            EncodingMode::ByteRange => 256,
            _ => self.chars.len(),
        }
    }
    
    /// Returns the encoding mode of this alphabet.
    pub fn mode(&self) -> &EncodingMode {
        &self.mode
    }
    
    /// Returns the padding character, if any.
    pub fn padding(&self) -> Option<char> {
        self.padding
    }
    
    /// Returns the starting Unicode codepoint for ByteRange mode.
    pub fn start_codepoint(&self) -> Option<u32> {
        self.start_codepoint
    }
    
    /// Encodes a digit (0 to base-1) as a character.
    ///
    /// Returns `None` if the digit is out of range.
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
    
    /// Decodes a character back to its digit value.
    ///
    /// Returns `None` if the character is not in the alphabet.
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


