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
    /// Output alphabet (encode to this alphabet)
    #[arg(short = 't', long)]
    to: Option<String>,
    
    /// Input alphabet (decode from this alphabet)
    #[arg(short = 'f', long)]
    from: Option<String>,
    
    /// File to encode/decode (if not provided, reads from stdin)
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
    
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
    
    // Determine operation mode based on flags
    match (&cli.from, &cli.to) {
        (Some(from_alphabet_name), Some(to_alphabet_name)) => {
            // Transcode mode: --from X --to Y
            if cli.stream {
                return Err("Streaming mode not yet supported for transcoding".into());
            }
            
            let from_alphabet = create_alphabet(from_alphabet_name)?;
            let to_alphabet = create_alphabet(to_alphabet_name)?;
            
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
        }
        
        (Some(from_alphabet_name), None) => {
            // Decode mode: --from X (decode to binary)
            let from_alphabet = create_alphabet(from_alphabet_name)?;
            
            if cli.stream {
                use base_d::StreamingDecoder;
                let mut decoder = StreamingDecoder::new(&from_alphabet, io::stdout());
                if let Some(file_path) = cli.file {
                    let mut file = fs::File::open(&file_path)?;
                    decoder.decode(&mut file)?;
                } else {
                    decoder.decode(&mut io::stdin())?;
                }
            } else {
                let input_data = if let Some(file_path) = cli.file {
                    fs::read_to_string(&file_path)?
                } else {
                    let mut buffer = String::new();
                    io::stdin().read_to_string(&mut buffer)?;
                    buffer
                };
                
                let decoded = decode(input_data.trim(), &from_alphabet)?;
                io::stdout().write_all(&decoded)?;
            }
        }
        
        (None, Some(to_alphabet_name)) => {
            // Encode mode: --to X (encode from binary)
            let to_alphabet = create_alphabet(to_alphabet_name)?;
            
            if cli.stream {
                use base_d::StreamingEncoder;
                let mut encoder = StreamingEncoder::new(&to_alphabet, io::stdout());
                if let Some(file_path) = cli.file {
                    let mut file = fs::File::open(&file_path)?;
                    encoder.encode(&mut file)?;
                } else {
                    encoder.encode(&mut io::stdin())?;
                }
            } else {
                let input_data = if let Some(file_path) = cli.file {
                    fs::read(&file_path)?
                } else {
                    let mut buffer = Vec::new();
                    io::stdin().read_to_end(&mut buffer)?;
                    buffer
                };
                
                let encoded = encode(&input_data, &to_alphabet);
                println!("{}", encoded);
            }
        }
        
        (None, None) => {
            // No alphabet specified - use default (cards) for encoding
            let to_alphabet = create_alphabet("cards")?;
            
            if cli.stream {
                use base_d::StreamingEncoder;
                let mut encoder = StreamingEncoder::new(&to_alphabet, io::stdout());
                if let Some(file_path) = cli.file {
                    let mut file = fs::File::open(&file_path)?;
                    encoder.encode(&mut file)?;
                } else {
                    encoder.encode(&mut io::stdin())?;
                }
            } else {
                let input_data = if let Some(file_path) = cli.file {
                    fs::read(&file_path)?
                } else {
                    let mut buffer = Vec::new();
                    io::stdin().read_to_end(&mut buffer)?;
                    buffer
                };
                
                let encoded = encode(&input_data, &to_alphabet);
                println!("{}", encoded);
            }
        }
    }
    
    Ok(())
}

