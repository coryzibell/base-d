use base_d::{decode, encode, Dictionary, DictionaryRegistry};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{poll, read, Event, KeyCode, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use super::config::{create_dictionary, get_compression_level, load_xxhash_config};

pub enum SwitchInterval {
    Time(Duration),
    PerLine,
}

pub enum SwitchMode {
    Static,
    Cycle(SwitchInterval),
    Random(SwitchInterval),
}

/// Parse interval string like "5s", "500ms", or "line"
pub fn parse_interval(s: &str) -> Result<SwitchInterval, Box<dyn std::error::Error>> {
    if s == "line" {
        return Ok(SwitchInterval::PerLine);
    }

    // Parse duration strings
    let s = s.trim();

    // Try to find where digits end
    let split_pos = s
        .chars()
        .position(|c| !c.is_ascii_digit())
        .ok_or("Invalid duration format")?;

    let (num_str, unit) = s.split_at(split_pos);
    let value: u64 = num_str.parse()?;

    let duration = match unit {
        "ms" => Duration::from_millis(value),
        "s" => Duration::from_secs(value),
        "m" => Duration::from_secs(value * 60),
        _ => return Err(format!("Unknown duration unit: {}", unit).into()),
    };

    Ok(SwitchInterval::Time(duration))
}

/// Select a random dictionary from the registry.
/// Only selects from dictionaries with `common=true` (renders consistently across platforms).
/// Optionally prints a warning message to stderr based on the `print_message` flag.
/// Returns the dictionary name.
pub fn select_random_dictionary(
    config: &DictionaryRegistry,
    print_message: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();

    // Filter to only common dictionaries (those that render consistently across platforms)
    let dict_names: Vec<&String> = config
        .dictionaries
        .iter()
        .filter(|(_, cfg)| cfg.common)
        .map(|(name, _)| name)
        .collect();

    let random_dict = dict_names
        .choose(&mut rng)
        .ok_or("No common dictionaries available")?;

    if print_message {
        eprintln!(
            "Note: Using randomly selected dictionary '{}' (use --encode={} to fix)",
            random_dict, random_dict
        );
    }

    Ok(random_dict.to_string())
}

/// Available hash algorithms for random selection
pub const HASH_ALGORITHMS: &[&str] = &["md5", "sha256", "sha512", "blake3", "xxh64", "xxh3"];

/// Available compression algorithms for random selection
pub const COMPRESS_ALGORITHMS: &[&str] = &["gzip", "zstd", "brotli", "lz4"];

/// Select a random hash algorithm
pub fn select_random_hash(quiet: bool) -> &'static str {
    use rand::seq::SliceRandom;
    let selected = HASH_ALGORITHMS.choose(&mut rand::thread_rng()).unwrap();
    if !quiet {
        eprintln!(
            "Note: Using randomly selected hash '{}' (use --hash={} to fix)",
            selected, selected
        );
    }
    selected
}

/// Select a random compression algorithm
pub fn select_random_compress(quiet: bool) -> &'static str {
    use rand::seq::SliceRandom;
    let selected = COMPRESS_ALGORITHMS.choose(&mut rand::thread_rng()).unwrap();
    if !quiet {
        eprintln!(
            "Note: Using randomly selected compression '{}' (use --compress={} to fix)",
            selected, selected
        );
    }
    selected
}

/// Matrix mode: Stream random data as Matrix-style falling code
pub fn matrix_mode(
    config: &DictionaryRegistry,
    initial_dictionary: &str,
    switch_mode: SwitchMode,
    no_color: bool,
    quiet: bool,
    superman: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::thread;
    use std::time::Instant;

    // Enable raw mode for keyboard input
    enable_raw_mode()?;

    if !no_color {
        print!("\x1b[2J\x1b[H"); // Clear screen
        print!("\x1b[32m"); // Green text
    }

    // Iconic Matrix messages
    let messages = [
        "Wake up, Neo...",
        "The Matrix has you...",
        "Follow the white rabbit.",
        "Knock, knock, Neo.",
    ];

    'intro_loop: for message in &messages {
        for ch in message.chars() {
            print!("{}", ch);
            io::stdout().flush()?;

            // Check for ESC/Space/Enter to skip intro
            if poll(Duration::from_millis(100))? {
                if let Event::Key(KeyEvent { code, .. }) = read()? {
                    if matches!(code, KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Enter) {
                        if !no_color {
                            print!("\r\x1b[K");
                        } else {
                            print!("\r");
                        }
                        break 'intro_loop;
                    }
                }
            } else {
                thread::sleep(Duration::from_millis(100));
            }
        }
        thread::sleep(Duration::from_millis(800));
        if !no_color {
            print!("\r\x1b[K");
        } else {
            print!("\r");
        }
        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(200));
    }

    thread::sleep(Duration::from_millis(500));

    // Setup for switching
    let mut current_dictionary_name = initial_dictionary.to_string();

    // For cycling: build sorted dictionary list
    let sorted_dicts: Vec<String> = if matches!(switch_mode, SwitchMode::Cycle(_)) {
        let mut names: Vec<String> = config.dictionaries.keys().cloned().collect();
        names.sort();
        names
    } else {
        Vec::new()
    };

    let mut cycle_index = sorted_dicts
        .iter()
        .position(|n| n == &current_dictionary_name)
        .unwrap_or(0);

    let mut last_switch = Instant::now();
    let mut rng = rand::thread_rng();

    // Build sorted dictionary list for keyboard controls
    let dict_names: Vec<String> = {
        let mut names: Vec<_> = config.dictionaries.keys().cloned().collect();
        names.sort();
        names
    };
    let mut current_index = dict_names
        .iter()
        .position(|n| n == &current_dictionary_name)
        .unwrap_or(0);

    // Display current dictionary name
    if !quiet {
        if !no_color {
            eprint!("\x1b[32mDictionary: {}\x1b[0m\r\n", current_dictionary_name);
        } else {
            eprint!("Dictionary: {}\r\n", current_dictionary_name);
        }
    }

    loop {
        // Load current dictionary
        let dictionary_config = config
            .get_dictionary(&current_dictionary_name)
            .ok_or(format!("{} dictionary not found", current_dictionary_name))?;

        let chars: Vec<char> = dictionary_config.chars.chars().collect();
        let dictionary = Dictionary::builder()
            .chars(chars)
            .mode(dictionary_config.mode.clone())
            .build()?;

        // Check if we need to switch (time-based)
        let should_switch = match &switch_mode {
            SwitchMode::Cycle(SwitchInterval::Time(d))
            | SwitchMode::Random(SwitchInterval::Time(d)) => last_switch.elapsed() >= *d,
            _ => false,
        };

        if should_switch {
            match &switch_mode {
                SwitchMode::Cycle(_) => {
                    cycle_index = (cycle_index + 1) % sorted_dicts.len();
                    current_dictionary_name = sorted_dicts[cycle_index].clone();
                }
                SwitchMode::Random(_) => {
                    current_dictionary_name = select_random_dictionary(config, false)?;
                }
                _ => {}
            }
            if !quiet {
                if !no_color {
                    eprint!("\x1b[32mDictionary: {}\x1b[0m\r\n", current_dictionary_name);
                } else {
                    eprint!("Dictionary: {}\r\n", current_dictionary_name);
                }
            }
            last_switch = Instant::now();
            continue; // Reload dictionary
        }

        // Check for line-based switching
        let switch_per_line = matches!(
            &switch_mode,
            SwitchMode::Cycle(SwitchInterval::PerLine)
                | SwitchMode::Random(SwitchInterval::PerLine)
        );

        if switch_per_line {
            match &switch_mode {
                SwitchMode::Cycle(SwitchInterval::PerLine) => {
                    cycle_index = (cycle_index + 1) % sorted_dicts.len();
                    current_dictionary_name = sorted_dicts[cycle_index].clone();
                }
                SwitchMode::Random(SwitchInterval::PerLine) => {
                    current_dictionary_name = select_random_dictionary(config, false)?;
                }
                _ => {}
            }
            if !quiet {
                if !no_color {
                    eprint!("\x1b[32mDictionary: {}\x1b[0m\r\n", current_dictionary_name);
                } else {
                    eprint!("Dictionary: {}\r\n", current_dictionary_name);
                }
            }
            continue; // Reload for next line
        }

        // Get current terminal width (re-check each line for resize detection)
        let term_width = match terminal_size::terminal_size() {
            Some((terminal_size::Width(w), _)) => w as usize,
            None => 80,
        };

        // Generate and encode one line
        // Calculate bytes needed to fill terminal width based on alphabet size
        // Each byte = 8 bits, each output char = log2(base) bits
        // bytes_needed = ceil(term_width * log2(base) / 8)
        let base = dictionary.base();
        let bits_per_char = (base as f64).log2();
        let bytes_per_line = ((term_width as f64 * bits_per_char) / 8.0).ceil() as usize;
        let mut random_bytes = vec![0u8; bytes_per_line.max(1)];

        use rand::RngCore;
        rng.fill_bytes(&mut random_bytes);

        let encoded = encode(&random_bytes, &dictionary);
        let display: String = encoded.chars().take(term_width).collect();

        print!("{}\r\n", display);
        io::stdout().flush()?;

        // Handle keyboard input (all modes)
        if poll(Duration::from_millis(25))? {
            if let Event::Key(key_event) = read()? {
                match key_event.code {
                    KeyCode::Char('c')
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        // Ctrl+C to exit
                        disable_raw_mode()?;
                        if !no_color {
                            print!("\x1b[0m"); // Reset color
                        }
                        std::process::exit(0);
                    }
                    KeyCode::Esc => {
                        // ESC to exit
                        disable_raw_mode()?;
                        if !no_color {
                            print!("\x1b[0m"); // Reset color
                        }
                        std::process::exit(0);
                    }
                    KeyCode::Char(' ') => {
                        // Random switch
                        current_dictionary_name = select_random_dictionary(config, false)?;
                        current_index = dict_names
                            .iter()
                            .position(|n| n == &current_dictionary_name)
                            .unwrap_or(0);
                        if !quiet {
                            if !no_color {
                                eprint!(
                                    "\r\x1b[32m[Matrix: {}]\x1b[0m\r\n",
                                    current_dictionary_name
                                );
                            } else {
                                eprint!("\r[Matrix: {}]\r\n", current_dictionary_name);
                            }
                        }
                        continue; // Reload dictionary
                    }
                    KeyCode::Left => {
                        // Previous dictionary
                        current_index = if current_index == 0 {
                            dict_names.len() - 1
                        } else {
                            current_index - 1
                        };
                        current_dictionary_name = dict_names[current_index].clone();
                        if !quiet {
                            if !no_color {
                                eprint!(
                                    "\r\x1b[32m[Matrix: {}]\x1b[0m\r\n",
                                    current_dictionary_name
                                );
                            } else {
                                eprint!("\r[Matrix: {}]\r\n", current_dictionary_name);
                            }
                        }
                        continue; // Reload dictionary
                    }
                    KeyCode::Right => {
                        // Next dictionary
                        current_index = (current_index + 1) % dict_names.len();
                        current_dictionary_name = dict_names[current_index].clone();
                        if !quiet {
                            if !no_color {
                                eprint!(
                                    "\r\x1b[32m[Matrix: {}]\x1b[0m\r\n",
                                    current_dictionary_name
                                );
                            } else {
                                eprint!("\r[Matrix: {}]\r\n", current_dictionary_name);
                            }
                        }
                        continue; // Reload dictionary
                    }
                    _ => {}
                }
            }
        }

        if !superman {
            thread::sleep(Duration::from_millis(250));
        }
    }
}

/// Auto-detect dictionary from input and decode
pub fn detect_mode(
    config: &DictionaryRegistry,
    file: Option<&PathBuf>,
    show_candidates: Option<usize>,
    decompress: Option<&String>,
    max_size: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use base_d::DictionaryDetector;

    // Read input
    let input = if let Some(file_path) = file {
        // Check file size BEFORE reading
        let metadata = fs::metadata(file_path)?;
        let file_size = metadata.len() as usize;

        if max_size > 0 && file_size > max_size {
            return Err(format!(
                "File size ({} bytes) exceeds limit ({} bytes). Use --force to process anyway.",
                file_size, max_size
            )
            .into());
        }

        fs::read_to_string(file_path)?
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;

        // Check stdin size AFTER reading (can't pre-check stdin)
        if max_size > 0 && buffer.len() > max_size {
            return Err(format!(
                "Input size ({} bytes) exceeds maximum ({} bytes). Use --file with --force for large inputs.",
                buffer.len(),
                max_size
            ).into());
        }

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
    if let Some(n) = show_candidates {
        println!("Top {} candidate dictionaries:\n", n);
        for (i, m) in matches.iter().take(n).enumerate() {
            println!(
                "{}. {} (confidence: {:.1}%)",
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
    eprintln!(
        "Detected: {} (confidence: {:.1}%)",
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
    let output = if let Some(decompress_name) = decompress {
        let algo = base_d::CompressionAlgorithm::from_str(decompress_name)?;
        base_d::decompress(&decoded, algo)?
    } else {
        decoded
    };

    // Output the decoded data
    io::stdout().write_all(&output)?;

    Ok(())
}

/// Streaming decode mode
#[allow(clippy::too_many_arguments)]
pub fn streaming_decode(
    config: &DictionaryRegistry,
    decode_name: &str,
    file: Option<&PathBuf>,
    decompress: Option<String>,
    hash: Option<String>,
    hash_seed: Option<u64>,
    hash_secret_stdin: bool,
    encode: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use base_d::StreamingDecoder;

    let decode_dictionary = create_dictionary(config, decode_name)?;
    let mut decoder = StreamingDecoder::new(&decode_dictionary, io::stdout());

    // Add decompression if specified
    if let Some(algo_name) = decompress {
        let algo = base_d::CompressionAlgorithm::from_str(&algo_name)?;
        decoder = decoder.with_decompression(algo);
    }

    // Add hashing if specified
    if let Some(hash_name) = &hash {
        let hash_algo = base_d::HashAlgorithm::from_str(hash_name)?;
        decoder = decoder.with_hashing(hash_algo);

        // Add xxHash config
        let xxhash_config =
            load_xxhash_config(hash_seed, hash_secret_stdin, config, Some(&hash_algo))?;
        decoder = decoder.with_xxhash_config(xxhash_config);
    }

    let hash_result = if let Some(file_path) = file {
        let mut file_handle = fs::File::open(file_path)?;
        decoder
            .decode(&mut file_handle)
            .map_err(|e| format!("{:?}", e))?
    } else {
        decoder
            .decode(&mut io::stdin())
            .map_err(|e| format!("{:?}", e))?
    };

    // Print hash if computed
    if let Some(hash_bytes) = hash_result {
        if let Some(encode_name) = encode {
            let encode_dictionary = create_dictionary(config, &encode_name)?;
            let hash_encoded = base_d::encode(&hash_bytes, &encode_dictionary);
            eprintln!("Hash: {}", hash_encoded);
        } else {
            eprintln!("Hash: {}", hex::encode(hash_bytes));
        }
    }

    Ok(())
}

/// Streaming encode mode
#[allow(clippy::too_many_arguments)]
pub fn streaming_encode(
    config: &DictionaryRegistry,
    encode_name: &str,
    file: Option<&PathBuf>,
    compress: Option<String>,
    level: Option<u32>,
    hash: Option<String>,
    hash_seed: Option<u64>,
    hash_secret_stdin: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use base_d::StreamingEncoder;

    let encode_dictionary = create_dictionary(config, encode_name)?;
    let mut encoder = StreamingEncoder::new(&encode_dictionary, io::stdout());

    // Add compression if specified
    if let Some(algo_name) = compress {
        let algo = base_d::CompressionAlgorithm::from_str(&algo_name)?;
        let compression_level = get_compression_level(config, level, algo);
        encoder = encoder.with_compression(algo, compression_level);
    }

    // Add hashing if specified
    if let Some(hash_name) = &hash {
        let hash_algo = base_d::HashAlgorithm::from_str(hash_name)?;
        encoder = encoder.with_hashing(hash_algo);

        // Add xxHash config
        let xxhash_config =
            load_xxhash_config(hash_seed, hash_secret_stdin, config, Some(&hash_algo))?;
        encoder = encoder.with_xxhash_config(xxhash_config);
    }

    let hash_result = if let Some(file_path) = file {
        let mut file_handle = fs::File::open(file_path)?;
        encoder
            .encode(&mut file_handle)
            .map_err(|e| format!("{}", e))?
    } else {
        encoder
            .encode(&mut io::stdin())
            .map_err(|e| format!("{}", e))?
    };

    // Print hash if computed
    if let Some(hash_bytes) = hash_result {
        eprintln!("Hash: {}", hex::encode(hash_bytes));
    }

    Ok(())
}
