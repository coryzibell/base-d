use crate::cli::{args::SchemaArgs, global::GlobalArgs};
use base_d::{DictionaryRegistry, decode_schema, encode_schema};
use std::fs;
use std::io::{self, Read};

pub fn handle(
    args: SchemaArgs,
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
        // Schema → JSON
        decode_schema(input_text.trim(), args.pretty)?
    } else {
        // JSON → Schema
        let compress = args.compress.map(Into::into);
        encode_schema(input_text.trim(), compress)?
    };

    // Write output
    if let Some(output_path) = &args.output {
        fs::write(output_path, output.as_bytes())?;
    } else {
        println!("{}", output);
    }

    Ok(())
}
