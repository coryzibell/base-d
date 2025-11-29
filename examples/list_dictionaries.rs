use base_d::DictionaryRegistry;

fn main() {
    let config = DictionaryRegistry::load_default().unwrap();

    println!("Available dictionaries:\n");

    for (name, dictionary_config) in config.dictionaries.iter() {
        let effective_mode = dictionary_config.effective_mode();
        let (char_count, preview) = match effective_mode {
            base_d::EncodingMode::ByteRange => {
                if let Some(start) = dictionary_config.start_codepoint {
                    let preview_chars: String = (0..10)
                        .filter_map(|i| std::char::from_u32(start + i))
                        .collect();
                    (256, preview_chars)
                } else {
                    (256, String::from("(invalid)"))
                }
            }
            _ => {
                let count = dictionary_config.chars.chars().count();
                let preview: String = dictionary_config.chars.chars().take(10).collect();
                (count, preview)
            }
        };
        let mode_str = match effective_mode {
            base_d::EncodingMode::Radix => "radix",
            base_d::EncodingMode::Chunked => "chunk",
            base_d::EncodingMode::ByteRange => "range",
        };
        println!(
            "  {} (base-{}, {}): {}...",
            name, char_count, mode_str, preview
        );
    }
}
