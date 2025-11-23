use base_d::{AlphabetsConfig, Alphabet, encode, decode};
use clap::Parser;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "base-d")]
#[command(about = "Encode and decode binary data using esoteric alphabets", long_about = None)]
struct Cli {
    /// Alphabet to use for encoding/decoding
    #[arg(short, long, default_value = "cards")]
    alphabet: String,
    
    /// File to encode/decode (if not provided, reads from stdin)
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
    
    /// Decode instead of encode
    #[arg(short, long)]
    decode: bool,
    
    /// List available alphabets
    #[arg(short, long)]
    list: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Load alphabets configuration with user overrides
    let config = AlphabetsConfig::load_with_overrides()?;
    
    // Handle list command
    if cli.list {
        println!("Available alphabets:\n");
        let mut alphabets: Vec<_> = config.alphabets.iter().collect();
        alphabets.sort_by_key(|(name, _)| *name);
        
        for (name, alphabet_config) in alphabets {
            let (char_count, preview) = match alphabet_config.mode {
                base_d::EncodingMode::ByteRange => {
                    if let Some(start) = alphabet_config.start_codepoint {
                        let preview_chars: String = (0..20)
                            .filter_map(|i| std::char::from_u32(start + i))
                            .collect();
                        (256, preview_chars)
                    } else {
                        (256, String::from("(invalid range)"))
                    }
                }
                _ => {
                    let count = alphabet_config.chars.chars().count();
                    let preview: String = alphabet_config.chars.chars().take(20).collect();
                    (count, preview)
                }
            };
            let suffix = if char_count > 20 { "..." } else { "" };
            let mode_str = match alphabet_config.mode {
                base_d::EncodingMode::BaseConversion => "math",
                base_d::EncodingMode::Chunked => "chunk",
                base_d::EncodingMode::ByteRange => "range",
            };
            println!("  {:<15} base-{:<3} {:>5}  {}{}", name, char_count, mode_str, preview, suffix);
        }
        return Ok(());
    }
    
    let alphabet_config = config.get_alphabet(&cli.alphabet)
        .ok_or_else(|| format!("Alphabet '{}' not found. Use --list to see available alphabets.", cli.alphabet))?;
    
    let alphabet = match alphabet_config.mode {
        base_d::EncodingMode::ByteRange => {
            let start = alphabet_config.start_codepoint
                .ok_or_else(|| "ByteRange mode requires start_codepoint")?;
            Alphabet::new_with_mode_and_range(Vec::new(), alphabet_config.mode.clone(), None, Some(start))
                .map_err(|e| format!("Invalid alphabet: {}", e))?
        }
        _ => {
            let chars: Vec<char> = alphabet_config.chars.chars().collect();
            let padding = alphabet_config.padding.as_ref().and_then(|s| s.chars().next());
            Alphabet::new_with_mode(chars, alphabet_config.mode.clone(), padding)
                .map_err(|e| format!("Invalid alphabet: {}", e))?
        }
    };
    
    // Read input data
    let input_data = if let Some(file_path) = cli.file {
        fs::read(&file_path)?
    } else {
        let mut buffer = Vec::new();
        io::stdin().read_to_end(&mut buffer)?;
        buffer
    };
    
    // Process based on mode
    if cli.decode {
        // Decode mode
        let input_str = String::from_utf8(input_data)
            .map_err(|_| "Input must be valid UTF-8 for decoding")?;
        let decoded = decode(input_str.trim(), &alphabet)?;
        io::stdout().write_all(&decoded)?;
    } else {
        // Encode mode
        let encoded = encode(&input_data, &alphabet);
        println!("{}", encoded);
    }
    
    Ok(())
}

