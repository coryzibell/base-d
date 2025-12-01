use crate::cli::{
    args::{ConfigAction, ConfigCategory},
    global::GlobalArgs,
};
use base_d::DictionaryRegistry;

/// Available hash algorithms for listing
const HASH_ALGORITHMS: &[&str] = &["md5", "sha256", "sha512", "blake3", "xxh64", "xxh3"];

/// Available compression algorithms for listing
const COMPRESS_ALGORITHMS: &[&str] = &["gzip", "zstd", "brotli", "lz4"];

pub fn handle(
    action: ConfigAction,
    _global: &GlobalArgs,
    config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ConfigAction::List { category, json } => handle_list(category, json, config),
        ConfigAction::Show { dictionary } => handle_show(&dictionary, config),
    }
}

fn handle_list(
    category: Option<ConfigCategory>,
    json: bool,
    config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    // Collect all data
    let compress_list = COMPRESS_ALGORITHMS.to_vec();
    let hash_list = HASH_ALGORITHMS.to_vec();
    let dict_list: Vec<String> = {
        let mut names: Vec<String> = config.dictionaries.keys().cloned().collect();
        names.sort();
        names
    };

    // JSON output
    if json {
        let output = match category {
            Some(ConfigCategory::Dictionaries) => serde_json::json!({ "dictionaries": dict_list }),
            Some(ConfigCategory::Algorithms) => serde_json::json!({ "algorithms": compress_list }),
            Some(ConfigCategory::Hashes) => serde_json::json!({ "hashes": hash_list }),
            None => serde_json::json!({
                "compression": compress_list,
                "hash": hash_list,
                "dictionaries": dict_list,
            }),
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // Comma-separated output for specific categories
    match category {
        Some(ConfigCategory::Dictionaries) => {
            println!("{}", dict_list.join(","));
        }
        Some(ConfigCategory::Algorithms) => {
            println!("{}", compress_list.join(","));
        }
        Some(ConfigCategory::Hashes) => {
            println!("{}", hash_list.join(","));
        }
        None => {
            // Human-readable format for all
            println!("Compression algorithms: {}", compress_list.join(", "));
            println!("Hash algorithms: {}", hash_list.join(", "));
            println!("Dictionaries: {} available", dict_list.len());
            println!(
                "\nUse 'config list dictionaries|algorithms|hashes' for machine-readable output"
            );
            println!("Use --json for structured output");
        }
    }

    Ok(())
}

fn handle_show(
    dict_name: &str,
    config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    let dict_config = config
        .dictionaries
        .get(dict_name)
        .ok_or_else(|| format!("Dictionary '{}' not found", dict_name))?;

    // Display dictionary details
    println!("Dictionary: {}", dict_name);

    // Show character set info
    if !dict_config.chars.is_empty() {
        println!("  Type: Explicit character set");
        println!("  Size: {} characters", dict_config.chars.chars().count());
        println!(
            "  Preview: {}...",
            dict_config.chars.chars().take(20).collect::<String>()
        );
    } else if let (Some(start), Some(length)) = (&dict_config.start, dict_config.length) {
        println!("  Type: Range-based");
        println!("  Start: {}", start);
        println!("  Length: {} characters", length);
    } else if let Some(codepoint) = dict_config.start_codepoint {
        println!("  Type: ByteRange");
        println!("  Start codepoint: U+{:04X}", codepoint);
        println!("  Length: 256 characters");
    } else {
        println!("  Type: Unknown configuration");
    }

    // Show mode
    if let Some(mode) = &dict_config.mode {
        println!("  Mode: {:?}", mode);
    }

    // Show padding
    if let Some(padding) = &dict_config.padding {
        println!("  Padding: {}", padding);
    }

    // Show common flag
    println!(
        "  Common: {}",
        if dict_config.common { "yes" } else { "no" }
    );

    Ok(())
}
