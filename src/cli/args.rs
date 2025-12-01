use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Arguments for encoding data
#[derive(Args, Debug)]
pub struct EncodeArgs {
    /// Dictionary to use for encoding
    pub dictionary: String,

    /// Input file (reads from stdin if not provided)
    pub file: Option<PathBuf>,

    /// Compress before encoding
    #[arg(short = 'c', long, value_name = "ALG")]
    pub compress: Option<Option<String>>,

    /// Compression level
    #[arg(long)]
    pub level: Option<u32>,

    /// Compute hash of input data
    #[arg(long, value_name = "ALG")]
    pub hash: Option<String>,

    /// Output file (writes to stdout if not provided)
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,

    /// Use streaming mode for large files
    #[arg(short = 's', long)]
    pub stream: bool,
}

/// Arguments for decoding data
#[derive(Args, Debug)]
pub struct DecodeArgs {
    /// Dictionary to decode from
    pub dictionary: String,

    /// Input file (reads from stdin if not provided)
    pub file: Option<PathBuf>,

    /// Decompress after decoding
    #[arg(long, value_name = "ALG")]
    pub decompress: Option<String>,

    /// Compute hash of decoded data
    #[arg(long, value_name = "ALG")]
    pub hash: Option<String>,

    /// Output file (writes to stdout if not provided)
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,

    /// Use streaming mode for large files
    #[arg(short = 's', long)]
    pub stream: bool,
}

/// Arguments for auto-detecting dictionary
#[derive(Args, Debug)]
pub struct DetectArgs {
    /// Input file (reads from stdin if not provided)
    pub file: Option<PathBuf>,

    /// Show top N candidate dictionaries
    #[arg(long, value_name = "N")]
    pub show_candidates: Option<usize>,

    /// Decompress after decoding
    #[arg(long)]
    pub decompress: Option<String>,
}

/// Arguments for hashing data
#[derive(Args, Debug)]
pub struct HashArgs {
    /// Hash algorithm to use
    pub algorithm: String,

    /// Input file (reads from stdin if not provided)
    pub file: Option<PathBuf>,

    /// Seed for xxHash algorithms
    #[arg(long)]
    pub seed: Option<u64>,

    /// Encode hash output using dictionary
    #[arg(long, value_name = "DICT")]
    pub encode: Option<String>,

    /// Read XXH3 secret from stdin
    #[arg(long)]
    pub secret_stdin: bool,
}

/// Arguments for Matrix mode
#[derive(Args, Debug)]
pub struct NeoArgs {
    /// Dictionary to use (default: base256_matrix)
    #[arg(long, value_name = "DICT")]
    pub dictionary: Option<String>,

    /// Use random dictionary
    #[arg(long)]
    pub dejavu: bool,

    /// Cycle through all dictionaries
    #[arg(long)]
    pub cycle: bool,

    /// Random dictionary switching
    #[arg(long)]
    pub random: bool,

    /// Switch interval (e.g., "5s", "500ms", "line")
    #[arg(long, value_name = "INTERVAL")]
    pub interval: Option<String>,

    /// Remove speed limit
    #[arg(long)]
    pub superman: bool,
}

/// Config subcommand actions
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// List available options
    List {
        /// What to list: dictionaries, algorithms, hashes
        #[arg(value_name = "TYPE")]
        category: Option<ConfigCategory>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show details for a specific dictionary
    Show {
        /// Dictionary name
        dictionary: String,
    },
}

/// Categories for config list command
#[derive(Clone, ValueEnum, Debug)]
pub enum ConfigCategory {
    Dictionaries,
    Algorithms,
    Hashes,
}
