// Keep utility modules for handlers
mod commands;
mod config;

// New modular CLI structure
pub mod args;
pub mod global;
pub mod handlers;

use base_d::DictionaryRegistry;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "base-d")]
#[command(version)]
#[command(about = "Universal multi-dictionary encoder supporting RFC standards, emoji, ancient scripts, and numerous custom dictionaries", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[command(flatten)]
    global: global::GlobalArgs,
}

#[derive(Subcommand)]
enum Commands {
    /// Encode data using a dictionary
    #[command(visible_alias = "e")]
    Encode(args::EncodeArgs),

    /// Decode data from a dictionary
    #[command(visible_alias = "d")]
    Decode(args::DecodeArgs),

    /// Auto-detect dictionary and decode
    Detect(args::DetectArgs),

    /// Compute hash of data
    Hash(args::HashArgs),

    /// Schema encoding: compact JSON representation (carrier98)
    Schema(args::SchemaArgs),

    /// Stele encoding: model-readable structured format
    Stele(args::SteleArgs),

    /// Query configuration and available options
    Config {
        #[command(subcommand)]
        action: args::ConfigAction,
    },

    /// Matrix mode: streaming visual effect
    Neo(args::NeoArgs),
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Load dictionaries configuration with user overrides
    let config = DictionaryRegistry::load_with_overrides()?;

    // Dispatch to appropriate handler
    match cli.command {
        Commands::Encode(args) => handlers::encode::handle(args, &cli.global, &config),
        Commands::Decode(args) => handlers::decode::handle(args, &cli.global, &config),
        Commands::Detect(args) => handlers::detect::handle(args, &cli.global, &config),
        Commands::Hash(args) => handlers::hash::handle(args, &cli.global, &config),
        Commands::Schema(args) => handlers::schema::handle(args, &cli.global, &config),
        Commands::Stele(args) => handlers::stele::handle(args, &cli.global, &config),
        Commands::Config { action } => handlers::config::handle(action, &cli.global, &config),
        Commands::Neo(args) => handlers::neo::handle(args, &cli.global, &config),
    }
}
