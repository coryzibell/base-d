/// Display-safe 96-character alphabet
///
/// Curated from Tank's research in ~/.matrix/ram/tank/final_recommendations.md
/// Conservative set excludes dashed variants for maximum display compatibility.
///
/// Character ranges:
/// - Box Drawing (heavy/double only): U+2501-U+257B
/// - Block Elements (full block + quadrants): U+2588-U+259F
/// - Geometric Shapes (solid filled only): U+25A0-U+25FF
///
/// All characters are:
/// - Visually distinct
/// - Cross-platform safe
/// - No blanks or confusables
/// - High contrast (solid fills or heavy lines)
/// - Size-independent rendering
// 96 characters from Tank's conservative recommendations (excluding dashed variants)
const DISPLAY96_CODEPOINTS: [u32; 96] = [
    // Box Drawing - Heavy variants (44 chars)
    0x2501, 0x2503, 0x250F, 0x2513, 0x2517, 0x251B, 0x2523, 0x252B, 0x2533, 0x253B, 0x254B, 0x2550,
    0x2551, 0x2552, 0x2553, 0x2554, 0x2555, 0x2556, 0x2557, 0x2558, 0x2559, 0x255A, 0x255B, 0x255C,
    0x255D, 0x255E, 0x255F, 0x2560, 0x2561, 0x2562, 0x2563, 0x2564, 0x2565, 0x2566, 0x2567, 0x2568,
    0x2569, 0x256A, 0x256B, 0x256C, 0x2578, 0x2579, 0x257A, 0x257B,
    // Block Elements - Full block + quadrants (11 chars)
    0x2588, 0x2596, 0x2597, 0x2598, 0x2599, 0x259A, 0x259B, 0x259C, 0x259D, 0x259E, 0x259F,
    // Geometric Shapes - Solid filled only (41 chars)
    0x25A0, 0x25A4, 0x25A5, 0x25A6, 0x25A7, 0x25A8, 0x25A9, 0x25AC, 0x25AE, 0x25B0, 0x25B2, 0x25B6,
    0x25BA, 0x25BB, 0x25BC, 0x25C0, 0x25C4, 0x25C5, 0x25C6, 0x25C9, 0x25CA, 0x25CD, 0x25CE, 0x25CF,
    0x25D4, 0x25D5, 0x25D8, 0x25DC, 0x25DD, 0x25DE, 0x25DF, 0x25E2, 0x25E3, 0x25E4, 0x25E5, 0x25EF,
    0x25F8, 0x25F9, 0x25FA, 0x25FC, 0x25FF,
];

/// Lazily-initialized display96 alphabet string
#[cfg(test)]
static DISPLAY96_ALPHABET: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Get the display96 alphabet as a string
#[cfg(test)]
pub fn alphabet() -> &'static str {
    DISPLAY96_ALPHABET.get_or_init(|| {
        DISPLAY96_CODEPOINTS
            .iter()
            .filter_map(|&cp| char::from_u32(cp))
            .collect()
    })
}

/// Get the character at a given index in the alphabet
pub fn char_at(index: usize) -> Option<char> {
    if index < 96 {
        char::from_u32(DISPLAY96_CODEPOINTS[index])
    } else {
        None
    }
}

/// Get the index of a character in the alphabet
pub fn index_of(c: char) -> Option<usize> {
    let codepoint = c as u32;
    DISPLAY96_CODEPOINTS.iter().position(|&cp| cp == codepoint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alphabet_length() {
        assert_eq!(alphabet().chars().count(), 96);
        assert_eq!(DISPLAY96_CODEPOINTS.len(), 96);
    }

    #[test]
    fn test_all_chars_valid() {
        // All codepoints should convert to valid chars
        for &cp in &DISPLAY96_CODEPOINTS {
            assert!(
                char::from_u32(cp).is_some(),
                "Invalid codepoint: U+{:04X}",
                cp
            );
        }
    }

    #[test]
    fn test_char_at() {
        assert_eq!(char_at(0), Some('━')); // U+2501
        assert_eq!(char_at(95), Some('◿')); // U+25FF - last char
        assert_eq!(char_at(96), None); // Out of bounds
    }

    #[test]
    fn test_index_of() {
        assert_eq!(index_of('━'), Some(0)); // U+2501
        assert_eq!(index_of('◿'), Some(95)); // U+25FF
        assert_eq!(index_of('A'), None); // Not in alphabet
    }

    #[test]
    fn test_roundtrip() {
        for i in 0..96 {
            let c = char_at(i).unwrap();
            let idx = index_of(c).unwrap();
            assert_eq!(idx, i, "Roundtrip failed for index {}", i);
        }
    }

    #[test]
    fn test_no_confusables() {
        let alpha = alphabet();

        // Should not contain ASCII confusables
        assert!(!alpha.contains('O'));
        assert!(!alpha.contains('0'));
        assert!(!alpha.contains('I'));
        assert!(!alpha.contains('l'));
        assert!(!alpha.contains('1'));

        // Should not contain whitespace
        assert!(!alpha.chars().any(|c| c.is_whitespace()));

        // All chars should be in expected ranges
        for c in alpha.chars() {
            let cp = c as u32;
            let in_range = (0x2501..=0x257B).contains(&cp) // Box drawing
                || (0x2588..=0x259F).contains(&cp)          // Block elements
                || (0x25A0..=0x25FF).contains(&cp); // Geometric shapes
            assert!(
                in_range,
                "Char '{}' (U+{:04X}) not in expected ranges",
                c, cp
            );
        }
    }

    #[test]
    fn test_visual_output() {
        println!("Display96 Alphabet:");
        println!("{}", alphabet());
        println!("\nLength: {} characters", alphabet().chars().count());
    }
}
