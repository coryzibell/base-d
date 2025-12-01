use crate::cli::{args::HashArgs, config::load_xxhash_config, global::GlobalArgs};
use base_d::DictionaryRegistry;
use std::fs;
use std::io::{self, Read, Write};

pub fn handle(
    args: HashArgs,
    global: &GlobalArgs,
    config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse hash algorithm
    let hash_algo = base_d::HashAlgorithm::from_str(&args.algorithm)?;

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

    // Load xxHash config (handles seed and secret)
    let xxhash_config = load_xxhash_config(args.seed, args.secret_stdin, config, Some(&hash_algo))?;

    // Compute hash
    let hash_output = base_d::hash_with_config(&input_data, hash_algo, &xxhash_config);

    // Output hash - either encoded or raw hex
    if let Some(encode_dict) = &args.encode {
        // Encode using specified dictionary
        let dictionary = crate::cli::config::create_dictionary(config, encode_dict)?;
        let encoded = base_d::encode(&hash_output, &dictionary);
        println!("{}", encoded);
    } else if global.raw {
        // Raw binary output
        io::stdout().write_all(&hash_output)?;
    } else {
        // Default: hex encoding
        println!("{}", hex::encode(&hash_output));
    }

    Ok(())
}
