//! Built-in word lists for word-based encoding.
//!
//! Currently includes:
//! - BIP-39 English (2048 words)
//! - EFF Long (7776 words)
//! - EFF Short 1 (1296 words)
//! - EFF Short 2 (1296 words)
//! - Diceware (7776 words)
//! - PGP Even (256 words, 2-syllable)
//! - PGP Odd (256 words, 3-syllable)
//! - NATO Phonetic Alphabet (26 words)
//! - Corporate Buzzwords (61 words)
//! - Klingon (3838 words)
//! - Pokemon (1092 words)

use crate::core::word_dictionary::WordDictionary;

// Security word lists
/// The BIP-39 English word list (2048 words).
/// Used for cryptocurrency seed phrases. Each word encodes 11 bits.
pub const BIP39_ENGLISH: &str = include_str!("../../dictionaries/word/security/bip39-english.txt");

/// The EFF Long word list (7776 words).
/// Improved diceware list with longer, more memorable words.
pub const EFF_LONG: &str = include_str!("../../dictionaries/word/security/eff-long.txt");

/// The EFF Short word list #1 (1296 words).
/// Shorter words, 4 dice rolls per word.
pub const EFF_SHORT1: &str = include_str!("../../dictionaries/word/security/eff-short1.txt");

/// The EFF Short word list #2 (1296 words).
/// Longer memorable words, 4 dice rolls per word.
pub const EFF_SHORT2: &str = include_str!("../../dictionaries/word/security/eff-short2.txt");

/// The original Diceware word list (7776 words).
/// Classic passphrase generation list by Arnold Reinhold.
pub const DICEWARE: &str = include_str!("../../dictionaries/word/security/diceware.txt");

/// PGP word list - even positions (256 words, 2-syllable).
/// Used for fingerprint verification.
pub const PGP_EVEN: &str = include_str!("../../dictionaries/word/security/pgp-even.txt");

/// PGP word list - odd positions (256 words, 3-syllable).
/// Used for fingerprint verification.
pub const PGP_ODD: &str = include_str!("../../dictionaries/word/security/pgp-odd.txt");

// Fun word lists
/// NATO Phonetic Alphabet (26 words).
/// Alfa, Bravo, Charlie... Used for radio communication.
pub const NATO: &str = include_str!("../../dictionaries/word/fun/nato.txt");

/// Corporate Buzzwords (61 words).
/// Synergy, leverage, paradigm... For your next meeting.
pub const BUZZWORDS: &str = include_str!("../../dictionaries/word/fun/buzzwords.txt");

/// Klingon words (3838 words).
/// tlhIngan Hol - from Star Trek.
pub const KLINGON: &str = include_str!("../../dictionaries/word/fun/klingon.txt");

/// Pokemon names (1092 words).
/// Gotta encode 'em all.
pub const POKEMON: &str = include_str!("../../dictionaries/word/fun/pokemon.txt");

/// Creates a WordDictionary from the built-in BIP-39 English word list.
///
/// # Example
///
/// ```
/// use base_d::wordlists::bip39_english;
/// use base_d::word;
///
/// let dict = bip39_english();
/// assert_eq!(dict.base(), 2048);
///
/// let encoded = word::encode(b"hello", &dict);
/// let decoded = word::decode(&encoded, &dict).unwrap();
/// assert_eq!(decoded, b"hello");
/// ```
pub fn bip39_english() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(BIP39_ENGLISH)
        .delimiter(" ")
        .case_sensitive(false)
        .build()
        .expect("BIP-39 English word list should be valid")
}

/// Creates a WordDictionary from the EFF Long word list (7776 words).
pub fn eff_long() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(EFF_LONG)
        .delimiter(" ")
        .case_sensitive(false)
        .build()
        .expect("EFF Long word list should be valid")
}

/// Creates a WordDictionary from the EFF Short word list #1 (1296 words).
pub fn eff_short1() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(EFF_SHORT1)
        .delimiter(" ")
        .case_sensitive(false)
        .build()
        .expect("EFF Short 1 word list should be valid")
}

/// Creates a WordDictionary from the EFF Short word list #2 (1296 words).
pub fn eff_short2() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(EFF_SHORT2)
        .delimiter(" ")
        .case_sensitive(false)
        .build()
        .expect("EFF Short 2 word list should be valid")
}

/// Creates a WordDictionary from the Diceware word list (7776 words).
pub fn diceware() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(DICEWARE)
        .delimiter(" ")
        .case_sensitive(false)
        .build()
        .expect("Diceware word list should be valid")
}

/// Creates a WordDictionary from the PGP even (2-syllable) word list (256 words).
pub fn pgp_even() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(PGP_EVEN)
        .delimiter("-")
        .case_sensitive(false)
        .build()
        .expect("PGP Even word list should be valid")
}

/// Creates a WordDictionary from the PGP odd (3-syllable) word list (256 words).
pub fn pgp_odd() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(PGP_ODD)
        .delimiter("-")
        .case_sensitive(false)
        .build()
        .expect("PGP Odd word list should be valid")
}

/// Creates a WordDictionary from the NATO phonetic alphabet (26 words).
pub fn nato() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(NATO)
        .delimiter("-")
        .case_sensitive(false)
        .build()
        .expect("NATO word list should be valid")
}

/// Creates a WordDictionary from corporate buzzwords (61 words).
pub fn buzzwords() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(BUZZWORDS)
        .delimiter(" ")
        .case_sensitive(false)
        .build()
        .expect("Buzzwords word list should be valid")
}

/// Creates a WordDictionary from Klingon words (3838 words).
pub fn klingon() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(KLINGON)
        .delimiter(" ")
        .case_sensitive(true) // Klingon has case-sensitive orthography
        .build()
        .expect("Klingon word list should be valid")
}

/// Creates a WordDictionary from Pokemon names (1092 words).
pub fn pokemon() -> WordDictionary {
    WordDictionary::builder()
        .words_from_str(POKEMON)
        .delimiter(" ")
        .case_sensitive(false)
        .build()
        .expect("Pokemon word list should be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bip39_english_word_count() {
        let dict = bip39_english();
        assert_eq!(dict.base(), 2048);
    }

    #[test]
    fn test_bip39_english_first_word() {
        let dict = bip39_english();
        assert_eq!(dict.encode_word(0), Some("abandon"));
    }

    #[test]
    fn test_bip39_english_last_word() {
        let dict = bip39_english();
        assert_eq!(dict.encode_word(2047), Some("zoo"));
    }

    #[test]
    fn test_bip39_english_roundtrip() {
        use crate::encoders::algorithms::word;

        let dict = bip39_english();
        let data = b"The quick brown fox jumps over the lazy dog";
        let encoded = word::encode(data, &dict);
        let decoded = word::decode(&encoded, &dict).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_bip39_english_case_insensitive() {
        let dict = bip39_english();
        assert_eq!(dict.decode_word("abandon"), Some(0));
        assert_eq!(dict.decode_word("ABANDON"), Some(0));
        assert_eq!(dict.decode_word("Abandon"), Some(0));
    }
}
