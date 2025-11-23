use base_d::AlphabetsConfig;

fn main() {
    let config = AlphabetsConfig::load_default().unwrap();
    
    println!("Available alphabets:\n");
    
    for (name, alphabet_config) in config.alphabets.iter() {
        let char_count = alphabet_config.chars.chars().count();
        let preview: String = alphabet_config.chars.chars().take(10).collect();
        let mode_str = match alphabet_config.mode {
            base_d::EncodingMode::BaseConversion => "math",
            base_d::EncodingMode::Chunked => "chunk",
        };
        println!("  {} (base-{}, {}): {}...", name, char_count, mode_str, preview);
    }
}
