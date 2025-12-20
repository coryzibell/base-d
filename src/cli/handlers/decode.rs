use crate::cli::{
    args::DecodeArgs,
    commands::streaming_decode,
    config::{BuiltDictionary, create_any_dictionary, load_xxhash_config},
    global::GlobalArgs,
};
use base_d::DictionaryRegistry;
use std::fs;
use std::io::{self, Read, Write};

pub fn handle(
    args: DecodeArgs,
    global: &GlobalArgs,
    config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    // Handle streaming mode separately
    if args.stream {
        let resolved_decompress = args.decompress.clone();
        let resolved_hash = args.hash.clone();

        return streaming_decode(
            config,
            &args.dictionary,
            args.file.as_ref(),
            resolved_decompress,
            resolved_hash,
            args.xxhash_seed,
            args.xxhash_secret_stdin,
            None, // encode - not supported in new CLI structure yet
        );
    }

    // Read input data (must be valid UTF-8 for decoding)
    let input_text = if let Some(file_path) = &args.file {
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

        fs::read_to_string(file_path)?
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;

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

    // Step 1: Decode using specified dictionary
    let built_dict = create_any_dictionary(config, &args.dictionary)?;
    let mut data = match &built_dict {
        BuiltDictionary::Char(dict) => base_d::decode(input_text.trim(), dict)?,
        BuiltDictionary::Word(dict) => base_d::word::decode(input_text.trim(), dict)?,
        BuiltDictionary::Alternating(dict) => base_d::word_alternating::decode(input_text.trim(), dict)?,
    };

    // Step 2: Decompress if requested
    if let Some(decompress_name) = &args.decompress {
        let decompress_algo = base_d::CompressionAlgorithm::from_str(decompress_name)?;
        data = base_d::decompress(&data, decompress_algo)?;
    }

    // Step 3: Compute hash if requested (hash of decoded data after decompression)
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

    // Step 4: Output decoded result
    if let Some(output_path) = &args.output {
        fs::write(output_path, &data)?;
    } else {
        io::stdout().write_all(&data)?;
    }

    // Step 5: Display hash if computed (to stderr, after main output)
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
