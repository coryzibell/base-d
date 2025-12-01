use crate::cli::commands::detect_mode;
use crate::cli::{args::DetectArgs, global::GlobalArgs};
use base_d::DictionaryRegistry;

pub fn handle(
    args: DetectArgs,
    global: &GlobalArgs,
    config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    // Determine max_size based on --force flag
    let max_size = if global.force { 0 } else { global.max_size };

    // Call the existing detect_mode function from commands.rs
    detect_mode(
        config,
        args.file.as_ref(),
        args.show_candidates,
        args.decompress.as_ref(),
        max_size,
    )
}
