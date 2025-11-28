mod commands;
mod config;

use base_d::{decode, encode, DictionaryRegistry};
use clap::Parser;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use config::{create_dictionary, get_compression_level, load_xxhash_config};

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
    /// If no algorithm specified, picks randomly
    #[arg(short = 'c', long, value_name = "ALGORITHM")]
    compress: Option<Option<String>>,

    /// Decompress data after decoding (gzip, zstd, brotli, lz4, snappy, lzma)
    #[arg(long)]
    decompress: Option<String>,

    /// Compression level (algorithm-specific, typically 1-9)
    #[arg(long)]
    level: Option<u32>,

    /// Compute hash of input data (md5, sha256, sha512, blake3, etc.)
    /// If no algorithm specified, picks randomly
    #[arg(long, value_name = "ALGORITHM")]
    hash: Option<Option<String>>,

    /// Seed for xxHash algorithms (u64, default: 0)
    #[arg(long)]
    hash_seed: Option<u64>,

    /// Read XXH3 secret from stdin (must be >= 136 bytes)
    #[arg(long)]
    hash_secret_stdin: bool,

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
    /// Can be combined with --dejavu to pick a random dictionary for Matrix mode
    #[arg(long, value_name = "DICTIONARY")]
    neo: Option<Option<String>>,

    /// Random dictionary encoding: Pick a random dictionary and encode with it
    /// When combined with --neo, uses random dictionary for Matrix mode
    #[arg(long, conflicts_with = "encode")]
    dejavu: bool,

    /// Cycle through all dictionaries in Matrix mode (requires --neo --dejavu)
    #[arg(long, requires = "dejavu")]
    cycle: bool,

    /// Random dictionary switching in Matrix mode (requires --neo --dejavu)
    #[arg(long, requires = "dejavu")]
    random: bool,

    /// Interval for switching: duration (e.g., "5s", "500ms") or "line"
    #[arg(long, value_name = "INTERVAL")]
    interval: Option<String>,
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Load dictionaries configuration with user overrides
    let config = DictionaryRegistry::load_with_overrides()?;

    // Handle --neo mode (Matrix effect)
    if let Some(alphabet_opt) = &cli.neo {
        // Validate conflicting flags
        if cli.cycle && cli.random {
            return Err("Cannot use both --cycle and --random together".into());
        }
        if cli.interval.is_some() && !cli.cycle && !cli.random {
            return Err("--interval requires either --cycle or --random".into());
        }

        // Determine switch mode
        let switch_mode = if cli.cycle {
            let interval = commands::parse_interval(cli.interval.as_deref().unwrap_or("5s"))?;
            commands::SwitchMode::Cycle(interval)
        } else if cli.random {
            let interval = commands::parse_interval(cli.interval.as_deref().unwrap_or("3s"))?;
            commands::SwitchMode::Random(interval)
        } else {
            // Static mode (allows keyboard controls)
            commands::SwitchMode::Static
        };

        // Determine initial alphabet
        // If --dejavu is set and no explicit alphabet, pick random
        let initial_alphabet = if cli.dejavu && alphabet_opt.is_none() {
            let random_dict = commands::select_random_dictionary(&config, false)?;
            // Silently select - the puzzle is figuring out which dictionary was used
            random_dict
        } else {
            alphabet_opt
                .as_deref()
                .unwrap_or("base256_matrix")
                .to_string()
        };

        return commands::matrix_mode(&config, &initial_alphabet, switch_mode);
    }

    // Handle --detect mode (auto-detect dictionary)
    if cli.detect {
        return commands::detect_mode(
            &config,
            cli.file.as_ref(),
            cli.show_candidates,
            cli.decompress.as_ref(),
        );
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
            println!(
                "  {:<15} base-{:<3} {:>5}  {}{}",
                name, char_count, mode_str, preview, suffix
            );
        }
        return Ok(());
    }

    // Parse compression algorithm - if flag present but no value, pick random
    let compress_algo = match &cli.compress {
        Some(Some(algo)) => Some(base_d::CompressionAlgorithm::from_str(algo)?),
        Some(None) => Some(base_d::CompressionAlgorithm::from_str(commands::select_random_compress())?),
        None => None,
    };

    let decompress_algo = cli
        .decompress
        .as_ref()
        .map(|s| base_d::CompressionAlgorithm::from_str(s))
        .transpose()?;

    // Validate flags
    if cli.raw && cli.encode.is_some() {
        return Err("Cannot use --raw with --encode (output would not be raw)".into());
    }

    // Handle streaming mode separately (now with compression/hashing support)
    if cli.stream {
        // Resolve optional hash/compress to concrete values for streaming
        let resolved_hash = match &cli.hash {
            Some(Some(name)) => Some(name.clone()),
            Some(None) => Some(commands::select_random_hash().to_string()),
            None => None,
        };
        let resolved_compress = match &cli.compress {
            Some(Some(name)) => Some(name.clone()),
            Some(None) => Some(commands::select_random_compress().to_string()),
            None => None,
        };

        if let Some(decode_name) = &cli.decode {
            return commands::streaming_decode(
                &config,
                decode_name,
                cli.file.as_ref(),
                cli.decompress,
                resolved_hash,
                cli.hash_seed,
                cli.hash_secret_stdin,
                cli.encode,
            );
        } else if let Some(encode_name) = &cli.encode {
            return commands::streaming_encode(
                &config,
                encode_name,
                cli.file.as_ref(),
                resolved_compress,
                cli.level,
                resolved_hash,
                cli.hash_seed,
                cli.hash_secret_stdin,
            );
        } else {
            return Err("Streaming mode requires either --encode or --decode".into());
        }
    }

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
        let decode_alphabet = create_dictionary(&config, decode_name)?;
        let text = String::from_utf8(data)
            .map_err(|_| "Input data is not valid UTF-8 text for decoding")?;
        data = decode(text.trim(), &decode_alphabet)?;
    }

    // Step 2: Decompress if requested
    if let Some(algo) = decompress_algo {
        data = base_d::decompress(&data, algo)?;
    }

    // Step 3: Hash if requested - if flag present but no value, pick random
    if let Some(hash_opt) = &cli.hash {
        let hash_name = match hash_opt {
            Some(name) => name.clone(),
            None => commands::select_random_hash().to_string(),
        };
        let hash_algo = base_d::HashAlgorithm::from_str(&hash_name)?;
        let xxhash_config = load_xxhash_config(
            cli.hash_seed,
            cli.hash_secret_stdin,
            &config,
            Some(&hash_algo),
        )?;
        let hash_output = base_d::hash_with_config(&data, hash_algo, &xxhash_config);

        // Encode the hash - explicit dict, dejavu, or default (random if no default)
        let dict_name = if let Some(encode_name) = &cli.encode {
            encode_name.clone()
        } else if cli.dejavu {
            commands::select_random_dictionary(&config, false)?
        } else if let Some(default) = &config.settings.default_dictionary {
            default.clone()
        } else {
            // No default configured - use random
            commands::select_random_dictionary(&config, false)?
        };
        let encode_alphabet = create_dictionary(&config, &dict_name)?;
        let encoded = encode(&hash_output, &encode_alphabet);
        println!("{}", encoded);
        return Ok(());
    }

    // Step 4: Compress if requested
    if let Some(algo) = compress_algo {
        let level = get_compression_level(&config, cli.level, algo);
        data = base_d::compress(&data, algo, level)?;
    }

    // Step 5: Encode if requested, or output raw/default
    if cli.raw {
        // Raw binary output
        io::stdout().write_all(&data)?;
    } else if cli.dejavu {
        // Random dictionary encoding
        let random_dict = commands::select_random_dictionary(&config, true)?;
        let encode_alphabet = create_dictionary(&config, &random_dict)?;
        let encoded = encode(&data, &encode_alphabet);
        println!("{}", encoded);
    } else if let Some(encode_name) = &cli.encode {
        // Explicit encoding
        let encode_alphabet = create_dictionary(&config, encode_name)?;
        let encoded = encode(&data, &encode_alphabet);
        println!("{}", encoded);
    } else if compress_algo.is_some() {
        // Compressed but no explicit encoding - use default or random
        let dict_name = if let Some(default) = &config.settings.default_dictionary {
            default.clone()
        } else {
            commands::select_random_dictionary(&config, false)?
        };
        let encode_alphabet = create_dictionary(&config, &dict_name)?;
        let encoded = encode(&data, &encode_alphabet);
        println!("{}", encoded);
    } else {
        // No compression, no encoding - output as-is (or use default encoding)
        if cli.decode.is_none() {
            // Encoding mode without explicit dictionary - use default or random
            let dict_name = if let Some(default) = &config.settings.default_dictionary {
                default.clone()
            } else {
                commands::select_random_dictionary(&config, false)?
            };
            let encode_alphabet = create_dictionary(&config, &dict_name)?;
            let encoded = encode(&data, &encode_alphabet);
            println!("{}", encoded);
        } else {
            // Decode-only mode
            io::stdout().write_all(&data)?;
        }
    }

    Ok(())
}
