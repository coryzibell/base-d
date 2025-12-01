use crate::cli::{
    args::NeoArgs,
    commands::{matrix_mode, parse_interval, select_random_dictionary, SwitchMode},
    global::GlobalArgs,
};
use base_d::DictionaryRegistry;

pub fn handle(
    args: NeoArgs,
    global: &GlobalArgs,
    config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    // Determine initial dictionary
    let initial_dictionary = if let Some(dict_name) = &args.dictionary {
        dict_name.clone()
    } else if args.dejavu {
        // Random dictionary mode - select one at start
        select_random_dictionary(config, !global.quiet)?
    } else {
        // Default to base256_matrix
        "base256_matrix".to_string()
    };

    // Determine switch mode
    let switch_mode = if args.cycle {
        // Cycle mode
        if let Some(interval_str) = &args.interval {
            let interval = parse_interval(interval_str)?;
            SwitchMode::Cycle(interval)
        } else {
            // Default to per-line cycling if no interval specified
            SwitchMode::Cycle(crate::cli::commands::SwitchInterval::PerLine)
        }
    } else if args.random {
        // Random switching mode
        if let Some(interval_str) = &args.interval {
            let interval = parse_interval(interval_str)?;
            SwitchMode::Random(interval)
        } else {
            // Default to per-line random if no interval specified
            SwitchMode::Random(crate::cli::commands::SwitchInterval::PerLine)
        }
    } else {
        // Static mode (no switching)
        SwitchMode::Static
    };

    // Call matrix_mode with resolved parameters
    matrix_mode(
        config,
        &initial_dictionary,
        switch_mode,
        global.no_color,
        global.quiet,
        args.superman,
    )
}
