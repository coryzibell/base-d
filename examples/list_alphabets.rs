use base_d::DictionariesConfig;

fn main() {
    let config = DictionariesConfig::load_default().unwrap();

    println!("Available dictionaries:\n");

    for (name, alphabet_config) in config.dictionaries.iter() {
        let (char_count, preview) = match alphabet_config.mode {
            base_d::EncodingMode::ByteRange => {
                if let Some(start) = alphabet_config.start_codepoint {
                    let preview_chars: String = (0..10)
                        .filter_map(|i| std::char::from_u32(start + i))
                        .collect();
                    (256, preview_chars)
                } else {
                    (256, String::from("(invalid)"))
                }
            }
            _ => {
                let count = alphabet_config.chars.chars().count();
                let preview: String = alphabet_config.chars.chars().take(10).collect();
                (count, preview)
            }
        };
        let mode_str = match alphabet_config.mode {
            base_d::EncodingMode::BaseConversion => "math",
            base_d::EncodingMode::Chunked => "chunk",
            base_d::EncodingMode::ByteRange => "range",
        };
        println!(
            "  {} (base-{}, {}): {}...",
            name, char_count, mode_str, preview
        );
    }
}
