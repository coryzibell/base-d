use crate::cli::{
    args::{FicheArgs, FicheCommand, FicheDecodeArgs, FicheEncodeArgs, FicheMode},
    global::GlobalArgs,
};
use base_d::{
    DetectedMode, DictionaryRegistry, decode_fiche, decode_fiche_path, detect_fiche_mode,
    encode_fiche, encode_fiche_ascii, encode_fiche_light, encode_fiche_path, encode_fiche_readable,
    encode_markdown_fiche, encode_markdown_fiche_ascii, encode_markdown_fiche_light,
    encode_markdown_fiche_markdown, encode_markdown_fiche_readable,
};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

pub fn handle(
    args: FicheArgs,
    _global: &GlobalArgs,
    _config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        Some(FicheCommand::Encode(encode_args)) => handle_encode(encode_args),
        Some(FicheCommand::Decode(decode_args)) => handle_decode(decode_args),
        None => {
            // Implicit encode mode with top-level args
            let encode_args = FicheEncodeArgs {
                mode: args.mode.unwrap_or_default(),
                output: args.output,
                input: args.input,
                multiline: args.multiline,
                markdown: args.markdown,
            };
            handle_encode(encode_args)
        }
    }
}

fn handle_encode(args: FicheEncodeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let input_text = read_input(args.input.as_deref())?;
    let minify = !args.multiline;

    // Auto-detect mode if set to Auto (only for JSON, not markdown)
    let mode = match args.mode {
        FicheMode::Auto if !args.markdown => {
            let detected = detect_fiche_mode(input_text.trim());
            match detected {
                DetectedMode::Full => FicheMode::Full,
                DetectedMode::Path => FicheMode::Path,
            }
        }
        FicheMode::Auto => FicheMode::None, // Default to readable for markdown
        other => other,
    };

    let output = if args.markdown {
        // Markdown document → fiche
        match mode {
            FicheMode::Auto => unreachable!("Auto should be resolved by now"),
            FicheMode::None => encode_markdown_fiche_readable(input_text.trim(), minify)?,
            FicheMode::Light => encode_markdown_fiche_light(input_text.trim(), minify)?,
            FicheMode::Full => encode_markdown_fiche(input_text.trim(), minify)?,
            FicheMode::Path => {
                return Err("Path mode is not supported for markdown input".into());
            }
            FicheMode::Ascii => encode_markdown_fiche_ascii(input_text.trim())?,
            FicheMode::Markdown => encode_markdown_fiche_markdown(input_text.trim())?,
        }
    } else {
        // JSON → fiche (existing behavior)
        match mode {
            FicheMode::Auto => unreachable!("Auto should be resolved by now"),
            FicheMode::None => encode_fiche_readable(input_text.trim(), minify)?,
            FicheMode::Light => encode_fiche_light(input_text.trim(), minify)?,
            FicheMode::Full => encode_fiche(input_text.trim(), minify)?,
            FicheMode::Path => encode_fiche_path(input_text.trim())?,
            FicheMode::Ascii => encode_fiche_ascii(input_text.trim())?,
            FicheMode::Markdown => {
                return Err("Markdown mode requires --markdown flag (markdown input only)".into());
            }
        }
    };

    write_output(&output, args.output.as_ref())?;
    Ok(())
}

fn handle_decode(args: FicheDecodeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let input_text = read_input(args.input.as_deref())?;
    let trimmed = input_text.trim();

    // Auto-detect path mode: has ┃ separators but no ◉ row markers
    let has_row_marker = trimmed.lines().any(|line| line.starts_with("◉"));
    let has_field_sep = trimmed.contains('┃');

    let is_path_mode = has_field_sep && !has_row_marker;

    let output = if is_path_mode {
        decode_fiche_path(trimmed)?
    } else {
        decode_fiche(trimmed, args.pretty)?
    };

    write_output(&output, args.output.as_ref())?;
    Ok(())
}

fn read_input(input: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(input_str) = input {
        // Try as file path first
        if let Ok(content) = fs::read_to_string(input_str) {
            Ok(content)
        } else {
            // Treat as literal input string
            Ok(input_str.to_string())
        }
    } else {
        // Read from stdin
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        Ok(buffer)
    }
}

fn write_output(
    content: &str,
    output_path: Option<&PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = output_path {
        fs::write(path, content.as_bytes())?;
    } else {
        println!("{}", content);
    }
    Ok(())
}
