use base_d::{decode, encode, Dictionary, DictionaryRegistry};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{poll, read, Event, KeyCode, KeyEvent};

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
/// Optionally prints a "dejavu: ..." message to stderr based on the `print_message` flag.
/// Returns the dictionary name.
pub fn select_random_dictionary(
    config: &DictionaryRegistry,
    print_message: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();

    let dict_names: Vec<&String> = config.dictionaries.keys().collect();
    let random_dict = dict_names
        .choose(&mut rng)
        .ok_or("No dictionaries available")?;

    // Silently select - the puzzle is figuring out which dictionary was used
    let _ = print_message; // Reserved for future verbose mode

    Ok(random_dict.to_string())
}

/// Available hash algorithms for random selection
pub const HASH_ALGORITHMS: &[&str] = &["md5", "sha256", "sha512", "blake3", "xxh64", "xxh3"];

/// Available compression algorithms for random selection
pub const COMPRESS_ALGORITHMS: &[&str] = &["gzip", "zstd", "brotli", "lz4"];

/// Select a random hash algorithm
pub fn select_random_hash() -> &'static str {
    use rand::seq::SliceRandom;
    HASH_ALGORITHMS.choose(&mut rand::thread_rng()).unwrap()
}

/// Select a random compression algorithm
pub fn select_random_compress() -> &'static str {
    use rand::seq::SliceRandom;
    COMPRESS_ALGORITHMS.choose(&mut rand::thread_rng()).unwrap()
}

/// Matrix mode: Stream random data as Matrix-style falling code
pub fn matrix_mode(
    config: &DictionaryRegistry,
    initial_dictionary: &str,
    switch_mode: SwitchMode,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::thread;
    use std::time::Instant;

    // Get terminal width
    let term_width = match terminal_size::terminal_size() {
        Some((terminal_size::Width(w), _)) => w as usize,
        None => 80,
    };

    println!("\x1b[2J\x1b[H"); // Clear screen
    println!("\x1b[32m"); // Green text

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

            // Check for ESC to skip intro
            if poll(Duration::from_millis(100))? {
                if let Event::Key(KeyEvent {
                    code: KeyCode::Esc, ..
                }) = read()?
                {
                    print!("\r\x1b[K");
                    break 'intro_loop;
                }
            } else {
                thread::sleep(Duration::from_millis(100));
            }
        }
        thread::sleep(Duration::from_millis(800));
        print!("\r\x1b[K");
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
    eprintln!("\x1b[32mDictionary: {}\x1b[0m", current_dictionary_name);

    loop {
        // Load current dictionary
        let dictionary_config = config
            .get_dictionary(&current_dictionary_name)
            .ok_or(format!("{} dictionary not found", current_dictionary_name))?;

        let chars: Vec<char> = dictionary_config.chars.chars().collect();
        let dictionary = Dictionary::new_with_mode(chars, dictionary_config.mode.clone(), None)?;

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
            eprintln!("\x1b[32mDictionary: {}\x1b[0m", current_dictionary_name);
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
            eprintln!("\x1b[32mDictionary: {}\x1b[0m", current_dictionary_name);
            continue; // Reload for next line
        }

        // Generate and encode one line
        let bytes_per_line = if dictionary.base() == 256 {
            term_width
        } else {
            term_width / 2
        };
        let mut random_bytes = vec![0u8; bytes_per_line];

        use rand::RngCore;
        rng.fill_bytes(&mut random_bytes);

        let encoded = encode(&random_bytes, &dictionary);
        let display: String = encoded.chars().take(term_width).collect();

        println!("{}", display);
        io::stdout().flush()?;

        // Handle keyboard input (static mode only)
        if matches!(switch_mode, SwitchMode::Static) && poll(Duration::from_millis(0))? {
            if let Event::Key(key_event) = read()? {
                match key_event.code {
                    KeyCode::Char(' ') => {
                        // Random switch
                        current_dictionary_name = select_random_dictionary(config, false)?;
                        current_index = dict_names
                            .iter()
                            .position(|n| n == &current_dictionary_name)
                            .unwrap_or(0);
                        eprintln!("\r\x1b[32m[Matrix: {}]\x1b[0m", current_dictionary_name);
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
                        eprintln!("\r\x1b[32m[Matrix: {}]\x1b[0m", current_dictionary_name);
                        continue; // Reload dictionary
                    }
                    KeyCode::Right => {
                        // Next dictionary
                        current_index = (current_index + 1) % dict_names.len();
                        current_dictionary_name = dict_names[current_index].clone();
                        eprintln!("\r\x1b[32m[Matrix: {}]\x1b[0m", current_dictionary_name);
                        continue; // Reload dictionary
                    }
                    _ => {}
                }
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}

/// Auto-detect dictionary from input and decode
pub fn detect_mode(
    config: &DictionaryRegistry,
    file: Option<&PathBuf>,
    show_candidates: Option<usize>,
    decompress: Option<&String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use base_d::DictionaryDetector;

    // Read input
    let input = if let Some(file_path) = file {
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
