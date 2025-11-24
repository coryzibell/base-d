use base_d::{DictionariesConfig, Dictionary, encode, decode};
use clap::Parser;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "base-d")]
#[command(version)]
#[command(about = "Universal multi-dictionary encoder supporting RFC standards, emoji, ancient scripts, and numerous custom dictionaries", long_about = None)]
struct Cli {
    /// Encode using this dictionary
    #[arg(short = 'e', long)]
    encode: Option<String>,
    
    /// Decode from this dictionary
    #[arg(short = 'd', long)]
    decode: Option<String>,
    
    /// Compress data before encoding (gzip, zstd, brotli, lz4)
    #[arg(short = 'c', long)]
    compress: Option<String>,
    
    /// Decompress data after decoding (gzip, zstd, brotli, lz4)
    #[arg(long)]
    decompress: Option<String>,
    
    /// Compression level (algorithm-specific, typically 1-9)
    #[arg(long)]
    level: Option<u32>,
    
    /// Output raw binary data (no encoding after compression)
    #[arg(short = 'r', long)]
    raw: bool,
    
    /// Auto-detect dictionary from input and decode
    #[arg(long)]
    detect: bool,
    
    /// Show top N candidate dictionaries when using --detect
    #[arg(long, value_name = "N")]
    show_candidates: Option<usize>,
    
    /// File to process (if not provided, reads from stdin)
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
    
    /// List available dictionaries
    #[arg(short, long)]
    list: bool,
    
    /// Use streaming mode for large files (memory efficient)
    #[arg(short, long)]
    stream: bool,
    
    /// Enter the Matrix: Stream random data as Matrix-style falling code
    /// Optionally specify a dictionary (default: base256_matrix)
    #[arg(long, value_name = "DICTIONARY")]
    neo: Option<Option<String>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Load dictionaries configuration with user overrides
    let config = DictionariesConfig::load_with_overrides()?;
    
    // Handle --neo mode (Matrix effect)
    if let Some(alphabet_opt) = &cli.neo {
        let alphabet_name = alphabet_opt.as_deref().unwrap_or("base256_matrix");
        return matrix_mode(&config, alphabet_name);
    }
    
    // Handle --detect mode (auto-detect dictionary)
    if cli.detect {
        return detect_mode(&config, &cli);
    }
    
    // Handle list command
    if cli.list {
        println!("Available dictionaries:\n");
        let mut alphabets: Vec<_> = config.dictionaries.iter().collect();
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
    
    // Helper function to create dictionary from config
    let create_alphabet = |name: &str| -> Result<Dictionary, Box<dyn std::error::Error>> {
        let alphabet_config = config.get_dictionary(name)
            .ok_or_else(|| format!("Dictionary '{}' not found. Use --list to see available dictionaries.", name))?;
        
        let dictionary = match alphabet_config.mode {
            base_d::EncodingMode::ByteRange => {
                let start = alphabet_config.start_codepoint
                    .ok_or_else(|| "ByteRange mode requires start_codepoint")?;
                Dictionary::new_with_mode_and_range(Vec::new(), alphabet_config.mode.clone(), None, Some(start))
                    .map_err(|e| format!("Invalid dictionary: {}", e))?
            }
            _ => {
                let chars: Vec<char> = alphabet_config.chars.chars().collect();
                let padding = alphabet_config.padding.as_ref().and_then(|s| s.chars().next());
                Dictionary::new_with_mode(chars, alphabet_config.mode.clone(), padding)
                    .map_err(|e| format!("Invalid dictionary: {}", e))?
            }
        };
        Ok(dictionary)
    };
    
    // Parse compression algorithms if provided
    let compress_algo = cli.compress.as_ref()
        .map(|s| base_d::CompressionAlgorithm::from_str(s))
        .transpose()?;
    
    let decompress_algo = cli.decompress.as_ref()
        .map(|s| base_d::CompressionAlgorithm::from_str(s))
        .transpose()?;
    
    // Validate flags
    if cli.stream && (compress_algo.is_some() || decompress_algo.is_some()) {
        return Err("Streaming mode is not yet supported with compression".into());
    }
    
    if cli.raw && cli.encode.is_some() {
        return Err("Cannot use --raw with --encode (output would not be raw)".into());
    }
    
    // Handle streaming mode separately (no compression support yet)
    if cli.stream {
        if let Some(decode_name) = &cli.decode {
            use base_d::StreamingDecoder;
            let decode_alphabet = create_alphabet(decode_name)?;
            let mut decoder = StreamingDecoder::new(&decode_alphabet, io::stdout());
            if let Some(file_path) = &cli.file {
                let mut file = fs::File::open(file_path)?;
                decoder.decode(&mut file)?;
            } else {
                decoder.decode(&mut io::stdin())?;
            }
            return Ok(());
        } else if let Some(encode_name) = &cli.encode {
            use base_d::StreamingEncoder;
            let encode_alphabet = create_alphabet(encode_name)?;
            let mut encoder = StreamingEncoder::new(&encode_alphabet, io::stdout());
            if let Some(file_path) = &cli.file {
                let mut file = fs::File::open(file_path)?;
                encoder.encode(&mut file)?;
            } else {
                encoder.encode(&mut io::stdin())?;
            }
            return Ok(());
        } else {
            return Err("Streaming mode requires either --encode or --decode".into());
        }
    }
    
    // Determine compression level
    let get_compression_level = |algo: base_d::CompressionAlgorithm| -> u32 {
        if let Some(level) = cli.level {
            level
        } else if let Some(comp_config) = config.compression.get(algo.as_str()) {
            comp_config.default_level
        } else {
            // Fallback defaults
            match algo {
                base_d::CompressionAlgorithm::Gzip => 6,
                base_d::CompressionAlgorithm::Zstd => 3,
                base_d::CompressionAlgorithm::Brotli => 6,
                base_d::CompressionAlgorithm::Lz4 => 0,
            }
        }
    };
    
    // Read input data
    let input_data = if let Some(file_path) = &cli.file {
        if cli.decode.is_some() {
            fs::read_to_string(file_path)?.into_bytes()
        } else {
            fs::read(file_path)?
        }
    } else {
        let mut buffer = Vec::new();
        io::stdin().read_to_end(&mut buffer)?;
        buffer
    };
    
    // Process data through pipeline
    let mut data = input_data;
    
    // Step 1: Decode if requested
    if let Some(decode_name) = &cli.decode {
        let decode_alphabet = create_alphabet(decode_name)?;
        let text = String::from_utf8(data)
            .map_err(|_| "Input data is not valid UTF-8 text for decoding")?;
        data = decode(text.trim(), &decode_alphabet)?;
    }
    
    // Step 2: Decompress if requested
    if let Some(algo) = decompress_algo {
        data = base_d::decompress(&data, algo)?;
    }
    
    // Step 3: Compress if requested
    if let Some(algo) = compress_algo {
        let level = get_compression_level(algo);
        data = base_d::compress(&data, algo, level)?;
    }
    
    // Step 4: Encode if requested, or output raw/default
    if cli.raw {
        // Raw binary output
        io::stdout().write_all(&data)?;
    } else if let Some(encode_name) = &cli.encode {
        // Explicit encoding
        let encode_alphabet = create_alphabet(encode_name)?;
        let encoded = encode(&data, &encode_alphabet);
        println!("{}", encoded);
    } else if compress_algo.is_some() {
        // Compressed but no explicit encoding - use default
        let default_dict = &config.settings.default_dictionary;
        let encode_alphabet = create_alphabet(default_dict)?;
        let encoded = encode(&data, &encode_alphabet);
        println!("{}", encoded);
    } else {
        // No compression, no encoding - output as-is (or use default encoding)
        if cli.decode.is_none() {
            // Encoding mode without explicit dictionary
            let encode_alphabet = create_alphabet("cards")?;
            let encoded = encode(&data, &encode_alphabet);
            println!("{}", encoded);
        } else {
            // Decode-only mode
            io::stdout().write_all(&data)?;
        }
    }
    
    Ok(())
}

fn matrix_mode(config: &DictionariesConfig, alphabet_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::thread;
    use std::time::Duration;
    
    // Load the specified dictionary
    let alphabet_config = config.get_dictionary(alphabet_name)
        .ok_or(format!("{} dictionary not found", alphabet_name))?;
    
    let chars: Vec<char> = alphabet_config.chars.chars().collect();
    let dictionary = Dictionary::new_with_mode(
        chars,
        alphabet_config.mode.clone(),
        None
    )?;
    
    // Get terminal width
    let term_width = match terminal_size::terminal_size() {
        Some((terminal_size::Width(w), _)) => w as usize,
        None => 80, // Default to 80 if we can't detect
    };
    
    println!("\x1b[2J\x1b[H"); // Clear screen and move to top
    println!("\x1b[32m"); // Green text
    
    // Iconic Matrix messages - typed character by character
    let messages = [
        "Wake up, Neo...",
        "The Matrix has you...",
        "Follow the white rabbit.",
        "Knock, knock, Neo.",
    ];
    
    for message in &messages {
        // Type out character by character
        for ch in message.chars() {
            print!("{}", ch);
            io::stdout().flush()?;
            thread::sleep(Duration::from_millis(100));
        }
        thread::sleep(Duration::from_millis(800)); // Pause to read
        
        // Clear the line
        print!("\r\x1b[K");
        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(200));
    }
    
    // Small pause before starting the stream
    thread::sleep(Duration::from_millis(500));
    
    // Cross-platform random source
    let mut rng = rand::thread_rng();
    
    loop {
        // Generate random bytes (one line worth)
        let bytes_per_line = term_width / 2; // Approximate, Matrix chars may be wider
        let mut random_bytes = vec![0u8; bytes_per_line];
        
        use rand::RngCore;
        rng.fill_bytes(&mut random_bytes);
        
        // Encode with Matrix dictionary
        let encoded = encode(&random_bytes, &dictionary);
        
        // Trim to terminal width (Matrix chars can be double-width)
        let display: String = encoded.chars().take(term_width).collect();
        
        // Print the line
        println!("{}", display);
        
        // Flush to ensure immediate display
        io::stdout().flush()?;
        
        // Sleep for half a second
        thread::sleep(Duration::from_millis(500));
    }
}

fn detect_mode(config: &DictionariesConfig, cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    use base_d::DictionaryDetector;
    
    // Read input
    let input = if let Some(file_path) = &cli.file {
        fs::read_to_string(file_path)?
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    };
    
    // Create detector and detect
    let detector = DictionaryDetector::new(config)?;
    let matches = detector.detect(input.trim());
    
    if matches.is_empty() {
        eprintln!("Could not detect dictionary - no matches found.");
        eprintln!("The input may not be encoded data, or uses an unknown dictionary.");
        std::process::exit(1);
    }
    
    // If --show-candidates is specified, show top N candidates
    if let Some(n) = cli.show_candidates {
        println!("Top {} candidate dictionaries:\n", n);
        for (i, m) in matches.iter().take(n).enumerate() {
            println!("{}. {} (confidence: {:.1}%)", 
                i + 1, 
                m.name, 
                m.confidence * 100.0
            );
        }
        return Ok(());
    }
    
    // Otherwise, use the best match to decode
    let best_match = &matches[0];
    
    // Show what was detected (to stderr so it doesn't interfere with output)
    eprintln!("Detected: {} (confidence: {:.1}%)", 
        best_match.name, 
        best_match.confidence * 100.0
    );
    
    // If confidence is low, warn the user
    if best_match.confidence < 0.7 {
        eprintln!("Warning: Low confidence detection. Results may be incorrect.");
    }
    
    // Decode using the detected dictionary
    let decoded = decode(input.trim(), &best_match.dictionary)?;
    
    // Handle decompression if requested
    let output = if let Some(decompress_name) = &cli.decompress {
        let algo = base_d::CompressionAlgorithm::from_str(decompress_name)?;
        base_d::decompress(&decoded, algo)?
    } else {
        decoded
    };
    
    // Output the decoded data
    io::stdout().write_all(&output)?;
    
    Ok(())
}

