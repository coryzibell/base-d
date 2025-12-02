use crate::cli::{args::FicheArgs, global::GlobalArgs};
use base_d::{DictionaryRegistry, decode_fiche, encode_fiche, encode_fiche_minified};
use std::fs;
use std::io::{self, Read};

pub fn handle(
    args: FicheArgs,
    _global: &GlobalArgs,
    _config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read input data
    let input_text = if let Some(file_path) = &args.file {
        fs::read_to_string(file_path)?
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    };

    // Process: encode or decode
    let output = if args.decode {
        // Fiche → JSON
        decode_fiche(input_text.trim(), args.pretty)?
    } else if args.minify {
        // JSON → Fiche (minified single line)
        encode_fiche_minified(input_text.trim())?
    } else {
        // JSON → Fiche
        encode_fiche(input_text.trim())?
    };

    // Write output
    if let Some(output_path) = &args.output {
        fs::write(output_path, output.as_bytes())?;
    } else {
        println!("{}", output);
    }

    Ok(())
}
