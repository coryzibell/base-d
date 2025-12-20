use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=dictionaries");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest_path = Path::new(&out_dir).join("registry.rs");
    let mut output = fs::File::create(&dest_path).expect("Failed to create registry.rs");

    // Start generating the registry code
    writeln!(
        output,
        "// Auto-generated dictionary registry from build.rs"
    )
    .unwrap();
    writeln!(output).unwrap();
    writeln!(
        output,
        "fn build_registry() -> HashMap<String, DictionaryConfig> {{"
    )
    .unwrap();
    writeln!(output, "    let mut map = HashMap::new();").unwrap();
    writeln!(output).unwrap();

    // Walk the dictionaries directory recursively
    let dict_dir = PathBuf::from("dictionaries");
    if dict_dir.exists() {
        process_directory(&dict_dir, &dict_dir, &mut output);
    }

    writeln!(output, "    map").unwrap();
    writeln!(output, "}}").unwrap();

    // Generate embedded word lists
    writeln!(output).unwrap();
    writeln!(
        output,
        "fn get_embedded_wordlist(filename: &str) -> Option<&'static str> {{"
    )
    .unwrap();
    writeln!(output, "    match filename {{").unwrap();

    if dict_dir.exists() {
        embed_wordlists(&dict_dir, &mut output);
    }

    writeln!(output, "        _ => None,").unwrap();
    writeln!(output, "    }}").unwrap();
    writeln!(output, "}}").unwrap();
}

fn process_directory(base_dir: &Path, current_dir: &Path, output: &mut fs::File) {
    let entries = match fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Warning: Failed to read directory {:?}: {}", current_dir, e);
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Warning: Failed to read entry: {}", e);
                continue;
            }
        };

        let path = entry.path();

        if path.is_dir() {
            process_directory(base_dir, &path, output);
        } else if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            process_toml_file(base_dir, &path, output);
        }
    }
}

fn process_toml_file(base_dir: &Path, toml_path: &Path, output: &mut fs::File) {
    // Generate dictionary name from path
    // e.g., "dictionaries/chunked/rfc/base64.toml" -> "base64"
    // or "dictionaries/radix/standards/base58.toml" -> "base58"
    let file_stem = toml_path
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("Invalid filename");

    // Replace hyphens with underscores in dictionary names
    let name = file_stem.replace('-', "_");

    // Get relative path for comments
    let rel_path = toml_path
        .strip_prefix(base_dir)
        .unwrap_or(toml_path)
        .to_string_lossy();

    writeln!(output, "    // {}", rel_path).unwrap();
    writeln!(output, "    {{").unwrap();
    writeln!(
        output,
        "        let toml_content = include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/dictionaries/{}\"));",
        rel_path
    ).unwrap();
    writeln!(
        output,
        "        let config: DictionaryConfig = toml::from_str(toml_content).expect(\"Failed to parse {}\");",
        name
    ).unwrap();
    writeln!(
        output,
        "        map.insert(\"{}\".to_string(), config);",
        name
    )
    .unwrap();
    writeln!(output, "    }}").unwrap();
    writeln!(output).unwrap();
}

fn embed_wordlists(base_dir: &Path, output: &mut fs::File) {
    // Walk dictionaries directory to find all .txt files
    let word_dir = base_dir.join("word");
    if !word_dir.exists() {
        return;
    }

    visit_dirs(&word_dir, &mut |txt_path: &Path| {
        if txt_path.extension().and_then(|s| s.to_str()) == Some("txt") {
            let filename = txt_path
                .file_name()
                .and_then(|s| s.to_str())
                .expect("Invalid filename");

            let rel_path = txt_path
                .strip_prefix(base_dir)
                .unwrap_or(txt_path)
                .to_string_lossy();

            writeln!(
                output,
                "        \"{}\" => Some(include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/dictionaries/{}\"))),",
                filename,
                rel_path
            ).unwrap();
        }
    }).expect("Failed to walk word directory");
}

fn visit_dirs(dir: &Path, cb: &mut dyn FnMut(&Path)) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&path);
            }
        }
    }
    Ok(())
}
