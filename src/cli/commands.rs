use base_d::{decode, encode, Dictionary, DictionaryRegistry};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use super::config::{create_dictionary, get_compression_level, load_xxhash_config};

/// Matrix mode: Stream random data as Matrix-style falling code
pub fn matrix_mode(
    config: &DictionaryRegistry,
    alphabet_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::thread;
    use std::time::Duration;

    // Load the specified dictionary
    let alphabet_config = config
        .get_dictionary(alphabet_name)
        .ok_or(format!("{} dictionary not found", alphabet_name))?;

    let chars: Vec<char> = alphabet_config.chars.chars().collect();
    let dictionary = Dictionary::new_with_mode(chars, alphabet_config.mode.clone(), None)?;

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

    let decode_alphabet = create_dictionary(config, decode_name)?;
    let mut decoder = StreamingDecoder::new(&decode_alphabet, io::stdout());

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
            let encode_alphabet = create_dictionary(config, &encode_name)?;
            let hash_encoded = base_d::encode(&hash_bytes, &encode_alphabet);
            eprintln!("Hash: {}", hash_encoded);
        } else {
            eprintln!("Hash: {}", hex::encode(hash_bytes));
        }
    }

    Ok(())
}

/// Streaming encode mode
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

    let encode_alphabet = create_dictionary(config, encode_name)?;
    let mut encoder = StreamingEncoder::new(&encode_alphabet, io::stdout());

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
