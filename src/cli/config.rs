use base_d::{Dictionary, DictionaryRegistry};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

/// Validates that a file path is within the allowed base-d config directory.
///
/// This prevents path traversal attacks by ensuring that user-provided file paths
/// (after tilde expansion) canonicalize to a location within `~/.config/base-d/`.
fn validate_config_path(path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let expanded = shellexpand::tilde(path);
    let canonical = fs::canonicalize(expanded.as_ref())
        .map_err(|e| format!("Cannot access path '{}': {}", path, e))?;

    let allowed_base = dirs::config_dir()
        .ok_or("Cannot determine config directory")?
        .join("base-d");

    if !canonical.starts_with(&allowed_base) {
        return Err(format!(
            "Path '{}' escapes allowed directory. Files must be within ~/.config/base-d/",
            path
        )
        .into());
    }

    Ok(canonical)
}

/// Helper function to create dictionary from config
pub fn create_dictionary(
    config: &DictionaryRegistry,
    name: &str,
) -> Result<Dictionary, Box<dyn std::error::Error>> {
    let dictionary_config = config.get_dictionary(name).ok_or_else(|| {
        // Try to find a close match
        let available: Vec<String> = config.dictionaries.keys().cloned().collect();
        let suggestion = base_d::find_closest_dictionary(name, &available);
        base_d::DictionaryNotFoundError::new(name, suggestion)
    })?;

    let effective_mode = dictionary_config.effective_mode();
    let dictionary = match effective_mode {
        base_d::EncodingMode::ByteRange => {
            let start = dictionary_config
                .start_codepoint
                .ok_or("ByteRange mode requires start_codepoint")?;
            Dictionary::builder()
                .chars(Vec::new())
                .mode(effective_mode)
                .start_codepoint(start)
                .build()
                .map_err(|e| format!("Invalid dictionary: {}", e))?
        }
        _ => {
            let chars: Vec<char> = dictionary_config
                .effective_chars()
                .map_err(|e| format!("Invalid dictionary config: {}", e))?
                .chars()
                .collect();
            let padding = dictionary_config
                .padding
                .as_ref()
                .and_then(|s| s.chars().next());
            let mut builder = Dictionary::builder().chars(chars).mode(effective_mode);
            if let Some(pad) = padding {
                builder = builder.padding(pad);
            }
            builder
                .build()
                .map_err(|e| format!("Invalid dictionary: {}", e))?
        }
    };
    Ok(dictionary)
}

/// Determine compression level from CLI args or config
pub fn get_compression_level(
    config: &DictionaryRegistry,
    cli_level: Option<u32>,
    algo: base_d::CompressionAlgorithm,
) -> u32 {
    if let Some(level) = cli_level {
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
            base_d::CompressionAlgorithm::Snappy => 0,
            base_d::CompressionAlgorithm::Lzma => 6,
        }
    }
}

/// Load xxHash configuration from CLI args and config file.
pub fn load_xxhash_config(
    cli_hash_seed: Option<u64>,
    cli_hash_secret_stdin: bool,
    config: &DictionaryRegistry,
    hash_algo: Option<&base_d::HashAlgorithm>,
) -> Result<base_d::XxHashConfig, Box<dyn std::error::Error>> {
    let seed = cli_hash_seed
        .or_else(|| {
            let default_seed = config.settings.xxhash.default_seed;
            if default_seed != 0 {
                Some(default_seed)
            } else {
                None
            }
        })
        .unwrap_or(0);

    let secret = if cli_hash_secret_stdin {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        Some(buf)
    } else if let Some(ref path) = config.settings.xxhash.default_secret_file {
        let validated_path = validate_config_path(path)?;
        Some(fs::read(validated_path)?)
    } else {
        None
    };

    // Warn if secret provided for non-XXH3
    if secret.is_some()
        && let Some(algo) = hash_algo
        && !matches!(
            algo,
            base_d::HashAlgorithm::XxHash3_64 | base_d::HashAlgorithm::XxHash3_128
        )
    {
        eprintln!(
            "Warning: --hash-secret-stdin only applies to xxh3-64/xxh3-128, ignoring for {}",
            algo.as_str()
        );
        return Ok(base_d::XxHashConfig::with_seed(seed));
    }

    match secret {
        Some(s) => base_d::XxHashConfig::with_secret(seed, s).map_err(|e| e.into()),
        None => Ok(base_d::XxHashConfig::with_seed(seed)),
    }
}
