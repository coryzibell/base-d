use std::fmt;

/// Errors that can occur during decoding.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The input contains a character not in the dictionary
    InvalidCharacter {
        char: char,
        position: usize,
        input: String,
        valid_chars: String,
    },
    /// The input string is empty
    EmptyInput,
    /// The padding is malformed or incorrect
    InvalidPadding,
    /// Invalid length for the encoding format
    InvalidLength {
        actual: usize,
        expected: String,
        hint: String,
    },
}

impl DecodeError {
    /// Create an InvalidCharacter error with context
    pub fn invalid_character(c: char, position: usize, input: &str, valid_chars: &str) -> Self {
        // Truncate long inputs
        let display_input = if input.len() > 60 {
            format!("{}...", &input[..60])
        } else {
            input.to_string()
        };

        DecodeError::InvalidCharacter {
            char: c,
            position,
            input: display_input,
            valid_chars: valid_chars.to_string(),
        }
    }

    /// Create an InvalidLength error
    pub fn invalid_length(
        actual: usize,
        expected: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        DecodeError::InvalidLength {
            actual,
            expected: expected.into(),
            hint: hint.into(),
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let use_color = should_use_color();

        match self {
            DecodeError::InvalidCharacter {
                char: c,
                position,
                input,
                valid_chars,
            } => {
                // Error header
                if use_color {
                    writeln!(
                        f,
                        "\x1b[1;31merror:\x1b[0m invalid character '{}' at position {}",
                        c, position
                    )?;
                } else {
                    writeln!(
                        f,
                        "error: invalid character '{}' at position {}",
                        c, position
                    )?;
                }
                writeln!(f)?;

                // Show input with caret pointing at error position
                // Need to account for multi-byte UTF-8 characters
                let char_position = input.chars().take(*position).count();
                writeln!(f, "  {}", input)?;
                write!(f, "  {}", " ".repeat(char_position))?;
                if use_color {
                    writeln!(f, "\x1b[1;31m^\x1b[0m")?;
                } else {
                    writeln!(f, "^")?;
                }
                writeln!(f)?;

                // Hint with valid characters (truncate if too long)
                let hint_chars = if valid_chars.len() > 80 {
                    format!("{}...", &valid_chars[..80])
                } else {
                    valid_chars.clone()
                };

                if use_color {
                    write!(f, "\x1b[1;36mhint:\x1b[0m valid characters: {}", hint_chars)?;
                } else {
                    write!(f, "hint: valid characters: {}", hint_chars)?;
                }
                Ok(())
            }
            DecodeError::EmptyInput => {
                if use_color {
                    write!(f, "\x1b[1;31merror:\x1b[0m cannot decode empty input")?;
                } else {
                    write!(f, "error: cannot decode empty input")?;
                }
                Ok(())
            }
            DecodeError::InvalidPadding => {
                if use_color {
                    writeln!(f, "\x1b[1;31merror:\x1b[0m invalid padding")?;
                    write!(
                        f,
                        "\n\x1b[1;36mhint:\x1b[0m check for missing or incorrect '=' characters at end of input"
                    )?;
                } else {
                    writeln!(f, "error: invalid padding")?;
                    write!(
                        f,
                        "\nhint: check for missing or incorrect '=' characters at end of input"
                    )?;
                }
                Ok(())
            }
            DecodeError::InvalidLength {
                actual,
                expected,
                hint,
            } => {
                if use_color {
                    writeln!(f, "\x1b[1;31merror:\x1b[0m invalid length for decode",)?;
                } else {
                    writeln!(f, "error: invalid length for decode")?;
                }
                writeln!(f)?;
                writeln!(f, "  input is {} characters, expected {}", actual, expected)?;
                writeln!(f)?;
                if use_color {
                    write!(f, "\x1b[1;36mhint:\x1b[0m {}", hint)?;
                } else {
                    write!(f, "hint: {}", hint)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Check if colored output should be used
fn should_use_color() -> bool {
    // Respect NO_COLOR environment variable
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }

    // Check if stderr is a terminal
    use std::io::IsTerminal;
    std::io::stderr().is_terminal()
}

/// Error when a dictionary is not found
#[derive(Debug)]
pub struct DictionaryNotFoundError {
    pub name: String,
    pub suggestion: Option<String>,
}

impl DictionaryNotFoundError {
    pub fn new(name: impl Into<String>, suggestion: Option<String>) -> Self {
        Self {
            name: name.into(),
            suggestion,
        }
    }
}

impl fmt::Display for DictionaryNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let use_color = should_use_color();

        if use_color {
            writeln!(
                f,
                "\x1b[1;31merror:\x1b[0m dictionary '{}' not found",
                self.name
            )?;
        } else {
            writeln!(f, "error: dictionary '{}' not found", self.name)?;
        }

        writeln!(f)?;

        if let Some(suggestion) = &self.suggestion {
            if use_color {
                writeln!(f, "\x1b[1;36mhint:\x1b[0m did you mean '{}'?", suggestion)?;
            } else {
                writeln!(f, "hint: did you mean '{}'?", suggestion)?;
            }
        }

        if use_color {
            write!(
                f,
                "      run \x1b[1m`base-d config --dictionaries`\x1b[0m to see all dictionaries"
            )?;
        } else {
            write!(
                f,
                "      run `base-d config --dictionaries` to see all dictionaries"
            )?;
        }

        Ok(())
    }
}

impl std::error::Error for DictionaryNotFoundError {}

/// Calculate Levenshtein distance between two strings
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut prev_row: Vec<usize> = (0..=len2).collect();
    let mut curr_row = vec![0; len2 + 1];

    for (i, c1) in s1.chars().enumerate() {
        curr_row[0] = i + 1;

        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            curr_row[j + 1] = (curr_row[j] + 1)
                .min(prev_row[j + 1] + 1)
                .min(prev_row[j] + cost);
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[len2]
}

/// Find the closest matching dictionary name
pub fn find_closest_dictionary(name: &str, available: &[String]) -> Option<String> {
    if available.is_empty() {
        return None;
    }

    let mut best_match = None;
    let mut best_distance = usize::MAX;

    for dict_name in available {
        let distance = levenshtein_distance(name, dict_name);

        // Only suggest if distance is reasonably small
        // (e.g., 1-2 character typos for short names, up to 3 for longer names)
        let threshold = if name.len() < 5 { 2 } else { 3 };

        if distance < best_distance && distance <= threshold {
            best_distance = distance;
            best_match = Some(dict_name.clone());
        }
    }

    best_match
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("base64", "base64"), 0);
        assert_eq!(levenshtein_distance("base64", "base32"), 2);
        assert_eq!(levenshtein_distance("bas64", "base64"), 1);
        assert_eq!(levenshtein_distance("", "base64"), 6);
    }

    #[test]
    fn test_find_closest_dictionary() {
        let dicts = vec![
            "base64".to_string(),
            "base32".to_string(),
            "base16".to_string(),
            "hex".to_string(),
        ];

        assert_eq!(
            find_closest_dictionary("bas64", &dicts),
            Some("base64".to_string())
        );
        assert_eq!(
            find_closest_dictionary("base63", &dicts),
            Some("base64".to_string())
        );
        assert_eq!(
            find_closest_dictionary("hex_radix", &dicts),
            None // too different
        );
    }

    #[test]
    fn test_error_display_no_color() {
        // Unsafe: environment variable access (not thread-safe)
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }

        let err = DecodeError::invalid_character('_', 12, "SGVsbG9faW52YWxpZA==", "A-Za-z0-9+/=");
        let display = format!("{}", err);

        assert!(display.contains("invalid character '_' at position 12"));
        assert!(display.contains("SGVsbG9faW52YWxpZA=="));
        assert!(display.contains("^"));
        assert!(display.contains("hint:"));

        // Unsafe: environment variable access (not thread-safe)
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe {
            std::env::remove_var("NO_COLOR");
        }
    }

    #[test]
    fn test_invalid_length_error() {
        // Unsafe: environment variable access (not thread-safe)
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }

        let err = DecodeError::invalid_length(
            13,
            "multiple of 4",
            "add padding (=) or check for missing characters",
        );
        let display = format!("{}", err);

        assert!(display.contains("invalid length"));
        assert!(display.contains("13 characters"));
        assert!(display.contains("multiple of 4"));
        assert!(display.contains("add padding"));

        // Unsafe: environment variable access (not thread-safe)
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe {
            std::env::remove_var("NO_COLOR");
        }
    }

    #[test]
    fn test_dictionary_not_found_error() {
        // Unsafe: environment variable access (not thread-safe)
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }

        let err = DictionaryNotFoundError::new("bas64", Some("base64".to_string()));
        let display = format!("{}", err);

        assert!(display.contains("dictionary 'bas64' not found"));
        assert!(display.contains("did you mean 'base64'?"));
        assert!(display.contains("base-d config --dictionaries"));

        // Unsafe: environment variable access (not thread-safe)
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe {
            std::env::remove_var("NO_COLOR");
        }
    }
}
