use base_d::{AlphabetsConfig, Alphabet, encode, decode};
use clap::Parser;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "base-d")]
#[command(version)]
#[command(about = "Universal multi-alphabet encoder supporting RFC standards, emoji, ancient scripts, and 33+ custom alphabets", long_about = None)]
struct Cli {
    /// Alphabet to use for encoding (or output alphabet when transcoding)
    #[arg(short, long, default_value = "cards")]
    alphabet: String,
    
    /// Input alphabet for transcoding (decode from this, encode to --alphabet)
    #[arg(short = 'f', long)]
    from: Option<String>,
    
    /// File to encode/decode (if not provided, reads from stdin)
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
    
    /// Decode instead of encode
    #[arg(short, long)]
    decode: bool,
    
    /// List available alphabets
    #[arg(short, long)]
    list: bool,
    
    /// Use streaming mode for large files (memory efficient)
    #[arg(short, long)]
    stream: bool,
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
    
    // Helper function to create alphabet from config
    let create_alphabet = |name: &str| -> Result<Alphabet, Box<dyn std::error::Error>> {
        let alphabet_config = config.get_alphabet(name)
            .ok_or_else(|| format!("Alphabet '{}' not found. Use --list to see available alphabets.", name))?;
        
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
        Ok(alphabet)
    };
    
    // Handle transcoding mode (--from specified)
    if let Some(from_alphabet_name) = &cli.from {
        if cli.decode {
            return Err("Cannot use --decode with --from (transcoding already decodes and encodes)".into());
        }
        if cli.stream {
            return Err("Streaming mode not yet supported for transcoding".into());
        }
        
        let from_alphabet = create_alphabet(from_alphabet_name)?;
        let to_alphabet = create_alphabet(&cli.alphabet)?;
        
        // Read input
        let input_data = if let Some(file_path) = cli.file {
            fs::read_to_string(&file_path)?
        } else {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        };
        
        // Transcode: decode from input alphabet, encode to output alphabet
        let decoded = decode(input_data.trim(), &from_alphabet)?;
        let encoded = encode(&decoded, &to_alphabet);
        println!("{}", encoded);
        
        return Ok(());
    }
    
    // Standard encode/decode mode
    let alphabet = create_alphabet(&cli.alphabet)?;
    
    // Process based on mode
    if cli.stream {
        // Streaming mode - process in chunks
        use base_d::{StreamingEncoder, StreamingDecoder};
        
        if cli.decode {
            let mut decoder = StreamingDecoder::new(&alphabet, io::stdout());
            if let Some(file_path) = cli.file {
                let mut file = fs::File::open(&file_path)?;
                decoder.decode(&mut file)?;
            } else {
                decoder.decode(&mut io::stdin())?;
            }
        } else {
            let mut encoder = StreamingEncoder::new(&alphabet, io::stdout());
            if let Some(file_path) = cli.file {
                let mut file = fs::File::open(&file_path)?;
                encoder.encode(&mut file)?;
            } else {
                encoder.encode(&mut io::stdin())?;
            }
        }
    } else {
        // Standard mode - load entire input into memory
        let input_data = if let Some(file_path) = cli.file {
            fs::read(&file_path)?
        } else {
            let mut buffer = Vec::new();
            io::stdin().read_to_end(&mut buffer)?;
            buffer
        };
        
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
    }
    
    Ok(())
}

