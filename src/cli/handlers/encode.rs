use crate::cli::{
    args::EncodeArgs,
    commands::{select_random_compress, streaming_encode},
    config::{BuiltDictionary, create_any_dictionary, get_compression_level, load_xxhash_config},
    global::GlobalArgs,
};
use base_d::DictionaryRegistry;
use std::fs;
use std::io::{self, Read, Write};

pub fn handle(
    args: EncodeArgs,
    global: &GlobalArgs,
    config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    // Handle streaming mode separately
    if args.stream {
        // Resolve optional compress/hash to concrete values for streaming
        let resolved_compress = match &args.compress {
            Some(Some(name)) => Some(name.clone()),
            Some(None) => Some(select_random_compress(global.quiet).to_string()),
            None => None,
        };

        let resolved_hash = args.hash.clone();

        return streaming_encode(
            config,
            &args.dictionary,
            args.file.as_ref(),
            resolved_compress,
            args.level,
            resolved_hash,
            args.xxhash_seed,
            args.xxhash_secret_stdin,
        );
    }

    // Read input data
    let input_data = if let Some(file_path) = &args.file {
        // Check file size if max_size is set
        if global.max_size > 0 {
            let metadata = fs::metadata(file_path)?;
            let file_size = metadata.len() as usize;

            if file_size > global.max_size {
                if global.force {
                    if !global.quiet {
                        eprintln!(
                            "Warning: Processing large file ({} bytes, limit: {} bytes)",
                            file_size, global.max_size
                        );
                    }
                } else {
                    return Err(format!(
                        "File size ({} bytes) exceeds limit ({} bytes). Use --force to process anyway.",
                        file_size, global.max_size
                    )
                    .into());
                }
            }
        }

        fs::read(file_path)?
    } else {
        let mut buffer = Vec::new();
        io::stdin().read_to_end(&mut buffer)?;

        // Check stdin size after reading
        if global.max_size > 0 && buffer.len() > global.max_size {
            return Err(format!(
                "Input size ({} bytes) exceeds maximum ({} bytes). Use --file with --force for large inputs.",
                buffer.len(),
                global.max_size
            )
            .into());
        }

        buffer
    };

    let mut data = input_data;

    // Step 1: Compute hash if requested (hash of input before compression/encoding)
    let hash_result = if let Some(hash_name) = &args.hash {
        let hash_algo = base_d::HashAlgorithm::from_str(hash_name)?;
        let xxhash_config = load_xxhash_config(
            args.xxhash_seed,
            args.xxhash_secret_stdin,
            config,
            Some(&hash_algo),
        )?;
        Some(base_d::hash_with_config(&data, hash_algo, &xxhash_config))
    } else {
        None
    };

    // Step 2: Compress if requested - resolve optional to concrete value
    let compress_algo = match &args.compress {
        Some(Some(algo)) => Some(base_d::CompressionAlgorithm::from_str(algo)?),
        Some(None) => Some(base_d::CompressionAlgorithm::from_str(
            select_random_compress(global.quiet),
        )?),
        None => None,
    };

    if let Some(algo) = compress_algo {
        let level = get_compression_level(config, args.level, algo);
        data = base_d::compress(&data, algo, level)?;
    }

    // Step 3: Encode using specified dictionary
    let built_dict = create_any_dictionary(config, &args.dictionary)?;
    let encoded = match &built_dict {
        BuiltDictionary::Char(dict) => base_d::encode(&data, dict),
        BuiltDictionary::Word(dict) => base_d::word::encode(&data, dict),
        BuiltDictionary::Alternating(dict) => base_d::word_alternating::encode(&data, dict)?,
    };

    // Step 4: Output encoded result
    if let Some(output_path) = &args.output {
        fs::write(output_path, encoded.as_bytes())?;
    } else {
        // Check for control characters before outputting
        if contains_control_chars(&encoded) {
            eprintln!("ERROR: Encoded output contains control characters");
            eprintln!("  Dictionary: {}", args.dictionary);
            eprintln!(
                "  Output bytes: {:?}",
                encoded.as_bytes().iter().take(50).collect::<Vec<_>>()
            );
            return Err(format!(
                "Encoded output contains control characters (dictionary: {})",
                args.dictionary
            )
            .into());
        }
        println!("{}", encoded);
    }

    // Step 5: Display hash if computed
    if let Some(hash_output) = hash_result {
        if global.raw {
            // Raw binary output to stderr
            io::stderr().write_all(&hash_output)?;
        } else {
            // Default: hex encoding to stderr
            eprintln!("Hash: {}", hex::encode(&hash_output));
        }
    }

    Ok(())
}

/// Check if output contains problematic control characters (0x00-0x1F except \t, \n, \r)
fn contains_control_chars(s: &str) -> bool {
    s.bytes()
        .any(|b| b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r')
}
