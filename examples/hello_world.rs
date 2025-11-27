use base_d::{decode, encode, DictionaryRegistry, Dictionary};

fn main() {
    let config = DictionaryRegistry::load_default().unwrap();
    let alphabet_config = config
        .get_dictionary("cards")
        .expect("cards dictionary not found");
    let chars: Vec<char> = alphabet_config.chars.chars().collect();
    let padding = alphabet_config
        .padding
        .as_ref()
        .and_then(|s| s.chars().next());
    let dictionary =
        Dictionary::new_with_mode(chars, alphabet_config.mode.clone(), padding).unwrap();

    let data = b"Hello, World!";

    println!("Original: {}", String::from_utf8_lossy(data));
    println!("Dictionary: cards (base-{})", dictionary.base());
    let encoded = encode(data, &dictionary);
    println!("Encoded:  {}", encoded);

    let decoded = decode(&encoded, &dictionary).unwrap();
    println!("Decoded:  {}", String::from_utf8_lossy(&decoded));
    println!("\nRoundtrip successful: {}", data == &decoded[..]);
}
