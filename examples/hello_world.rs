use base_d::{Dictionary, DictionaryRegistry, decode, encode};

fn main() {
    let config = DictionaryRegistry::load_default().unwrap();
    let dictionary_config = config
        .get_dictionary("cards")
        .expect("cards dictionary not found");
    let chars: Vec<char> = dictionary_config.chars.chars().collect();
    let padding = dictionary_config
        .padding
        .as_ref()
        .and_then(|s| s.chars().next());
    let mut builder = Dictionary::builder()
        .chars(chars)
        .mode(dictionary_config.effective_mode());
    if let Some(pad) = padding {
        builder = builder.padding(pad);
    }
    let dictionary = builder.build().unwrap();

    let data = b"Hello, World!";

    println!("Original: {}", String::from_utf8_lossy(data));
    println!("Dictionary: cards (base-{})", dictionary.base());
    let encoded = encode(data, &dictionary);
    println!("Encoded:  {}", encoded);

    let decoded = decode(&encoded, &dictionary).unwrap();
    println!("Decoded:  {}", String::from_utf8_lossy(&decoded));
    println!("\nRoundtrip successful: {}", data == &decoded[..]);
}
