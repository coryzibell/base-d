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

    /// Seed for xxHash algorithms
    #[arg(long)]
    pub xxhash_seed: Option<u64>,

    /// Read XXH3 secret from stdin
    #[arg(long)]
    pub xxhash_secret_stdin: bool,

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

    /// Seed for xxHash algorithms
    #[arg(long)]
    pub xxhash_seed: Option<u64>,

    /// Read XXH3 secret from stdin
    #[arg(long)]
    pub xxhash_secret_stdin: bool,

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

/// Arguments for schema encoding/decoding
#[derive(Args, Debug)]
pub struct SchemaArgs {
    /// Input file (reads from stdin if not provided)
    pub file: Option<PathBuf>,

    /// Decode mode (schema → JSON)
    #[arg(short = 'd', long)]
    pub decode: bool,

    /// Pretty-print JSON output (decode only)
    #[arg(short = 'p', long)]
    pub pretty: bool,

    /// Compression algorithm (default: none)
    #[arg(short = 'c', long, value_enum)]
    pub compress: Option<SchemaCompressionAlgoCli>,

    /// Output file (writes to stdout if not provided)
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,
}

/// Compression algorithms for schema encoding (CLI enum)
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SchemaCompressionAlgoCli {
    Brotli,
    Lz4,
    Zstd,
}

impl From<SchemaCompressionAlgoCli> for base_d::SchemaCompressionAlgo {
    fn from(cli: SchemaCompressionAlgoCli) -> Self {
        match cli {
            SchemaCompressionAlgoCli::Brotli => base_d::SchemaCompressionAlgo::Brotli,
            SchemaCompressionAlgoCli::Lz4 => base_d::SchemaCompressionAlgo::Lz4,
            SchemaCompressionAlgoCli::Zstd => base_d::SchemaCompressionAlgo::Zstd,
        }
    }
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

/// Stele encoding modes
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum SteleMode {
    /// Auto-detect best mode based on input structure
    #[default]
    Auto,
    /// No tokenization - human readable field names
    None,
    /// Field names only (runic tokens)
    Light,
    /// Field names + repeated values (runic + hieroglyphs)
    Full,
    /// Path mode - one line per leaf value with full path
    Path,
    /// ASCII mode - inline CSV-like format with value dictionary (best for JSON)
    Ascii,
    /// Markdown-like inline format (best for markdown input)
    Markdown,
}

/// Arguments for stele encoding/decoding (model-readable format)
#[derive(Args, Debug)]
pub struct SteleArgs {
    #[command(subcommand)]
    pub command: Option<SteleCommand>,

    // Top-level args for implicit encode
    /// Encoding mode
    #[arg(short, long)]
    pub mode: Option<SteleMode>,

    /// Output file (writes to stdout if not provided)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Input string or file (reads from stdin if not provided)
    pub input: Option<String>,

    /// Use multiline output format
    #[arg(long)]
    pub multiline: bool,

    /// Parse input as markdown document instead of JSON
    #[arg(long)]
    pub markdown: bool,
}

/// Stele subcommands
#[derive(Subcommand, Debug)]
pub enum SteleCommand {
    /// Encode JSON to stele format
    Encode(SteleEncodeArgs),
    /// Decode stele to JSON
    Decode(SteleDecodeArgs),
}

/// Arguments for stele encoding
#[derive(Args, Debug)]
pub struct SteleEncodeArgs {
    /// Encoding mode
    #[arg(short, long, default_value = "auto")]
    pub mode: SteleMode,

    /// Output file (writes to stdout if not provided)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Input string or file (reads from stdin if not provided)
    pub input: Option<String>,

    /// Use multiline output format
    #[arg(long)]
    pub multiline: bool,

    /// Parse input as markdown document instead of JSON
    #[arg(long)]
    pub markdown: bool,
}

/// Arguments for stele decoding
#[derive(Args, Debug)]
pub struct SteleDecodeArgs {
    /// Pretty-print JSON output
    #[arg(short, long)]
    pub pretty: bool,

    /// Output file (writes to stdout if not provided)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Input string or file (reads from stdin if not provided)
    pub input: Option<String>,
}
