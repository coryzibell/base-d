use std::collections::HashMap;
use crate::config::EncodingMode;

const MAX_LOOKUP_TABLE_SIZE: usize = 256;

/// Represents an encoding alphabet with its characters and configuration.
///
/// An alphabet defines the character set and encoding mode used for converting
/// binary data to text. Supports three modes: mathematical base conversion,
/// chunked (RFC 4648), and byte-range mapping.
#[derive(Debug, Clone)]
pub struct Alphabet {
    chars: Vec<char>,
    char_to_index: HashMap<char, usize>,
    // Fast lookup table for ASCII/extended ASCII characters
    lookup_table: Option<Box<[Option<usize>; 256]>>,
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
                    // Validate all codepoints in range are valid Unicode
                    for offset in 0..=255 {
                        if std::char::from_u32(start + offset).is_none() {
                            return Err(format!("Invalid Unicode codepoint in range: {}", start + offset));
                        }
                    }
                } else {
                    return Err("Start codepoint too high for 256-byte range".to_string());
                }
                
                return Ok(Alphabet {
                    chars: Vec::new(),
                    char_to_index: HashMap::new(),
                    lookup_table: None,
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
            // Additional check: ensure we have valid sizes for chunked mode
            if base != 2 && base != 4 && base != 8 && base != 16 && base != 32 && base != 64 && base != 128 && base != 256 {
                return Err(format!("Chunked mode requires alphabet size of 2, 4, 8, 16, 32, 64, 128, or 256, got {}", base));
            }
        }
        
        // Validate character properties
        let mut char_to_index = HashMap::new();
        for (i, &c) in chars.iter().enumerate() {
            // Check for duplicate characters
            if char_to_index.insert(c, i).is_some() {
                return Err(format!("Duplicate character in alphabet: '{}' (U+{:04X})", c, c as u32));
            }
            
            // Check for invalid Unicode characters
            if c.is_control() && c != '\t' && c != '\n' && c != '\r' {
                return Err(format!("Control character not allowed in alphabet: U+{:04X}", c as u32));
            }
            
            // Check for whitespace (except in specific cases)
            if c.is_whitespace() {
                return Err(format!("Whitespace character not allowed in alphabet: '{}' (U+{:04X})", c, c as u32));
            }
        }
        
        // Validate padding character if present
        if let Some(pad) = padding {
            if char_to_index.contains_key(&pad) {
                return Err(format!("Padding character '{}' conflicts with alphabet characters", pad));
            }
            if pad.is_control() && pad != '\t' && pad != '\n' && pad != '\r' {
                return Err(format!("Control character not allowed as padding: U+{:04X}", pad as u32));
            }
        }
        
        // Build fast lookup table for ASCII characters
        let lookup_table = if chars.iter().all(|&c| (c as u32) < MAX_LOOKUP_TABLE_SIZE as u32) {
            let mut table = Box::new([None; 256]);
            for (i, &c) in chars.iter().enumerate() {
                table[c as usize] = Some(i);
            }
            Some(table)
        } else {
            None
        };
        
        Ok(Alphabet {
            chars,
            char_to_index,
            lookup_table,
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
            _ => {
                // Use fast lookup table for ASCII characters
                if let Some(ref table) = self.lookup_table {
                    let char_val = c as u32;
                    if char_val < MAX_LOOKUP_TABLE_SIZE as u32 {
                        return table[char_val as usize];
                    }
                }
                // Fall back to HashMap for non-ASCII
                self.char_to_index.get(&c).copied()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_duplicate_character_detection() {
        let chars = vec!['a', 'b', 'c', 'a'];
        let result = Alphabet::new(chars);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate character"));
    }
    
    #[test]
    fn test_empty_alphabet() {
        let chars = vec![];
        let result = Alphabet::new(chars);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }
    
    #[test]
    fn test_chunked_mode_power_of_two() {
        let chars = vec!['a', 'b', 'c'];  // 3 is not power of 2
        let result = Alphabet::new_with_mode(chars, EncodingMode::Chunked, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("power-of-two"));
    }
    
    #[test]
    fn test_chunked_mode_valid_sizes() {
        // Test all valid chunked sizes
        for &size in &[2, 4, 8, 16, 32, 64] {
            let chars: Vec<char> = (0..size).map(|i| {
                // Use a wider range of Unicode characters
                char::from_u32('A' as u32 + (i % 26) + ((i / 26) * 100)).unwrap()
            }).collect();
            let result = Alphabet::new_with_mode(chars, EncodingMode::Chunked, None);
            assert!(result.is_ok(), "Size {} should be valid", size);
        }
    }
    
    #[test]
    fn test_control_character_rejection() {
        let chars = vec!['a', 'b', '\x00', 'c'];  // null character
        let result = Alphabet::new(chars);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Control character"));
    }
    
    #[test]
    fn test_whitespace_rejection() {
        let chars = vec!['a', 'b', ' ', 'c'];
        let result = Alphabet::new(chars);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Whitespace"));
    }
    
    #[test]
    fn test_padding_conflict_with_alphabet() {
        let chars = vec!['a', 'b', 'c', 'd'];
        let result = Alphabet::new_with_mode(chars, EncodingMode::BaseConversion, Some('b'));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Padding character"));
        assert!(err.contains("conflicts"));
    }
    
    #[test]
    fn test_valid_padding() {
        let chars = vec!['a', 'b', 'c', 'd'];
        let result = Alphabet::new_with_mode(chars, EncodingMode::BaseConversion, Some('='));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_byte_range_exceeds_unicode() {
        // Test with a start codepoint so high that start + 255 exceeds max valid Unicode (0x10FFFF)
        let result = Alphabet::new_with_mode_and_range(
            Vec::new(),
            EncodingMode::ByteRange,
            None,
            Some(0x10FF80)  // 0x10FF80 + 255 = 0x110078, exceeds 0x10FFFF
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_byte_range_valid_start() {
        let result = Alphabet::new_with_mode_and_range(
            Vec::new(),
            EncodingMode::ByteRange,
            None,
            Some(0x1F300)  // Valid start in emoji range
        );
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_byte_range_no_start_codepoint() {
        let result = Alphabet::new_with_mode_and_range(
            Vec::new(),
            EncodingMode::ByteRange,
            None,
            None
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires start_codepoint"));
    }
    
    #[test]
    fn test_detailed_error_messages() {
        // Test that error messages include useful information
        let chars = vec!['a', 'b', 'a'];
        let err = Alphabet::new(chars).unwrap_err();
        assert!(err.contains("'a'") || err.contains("U+"));
    }
}
