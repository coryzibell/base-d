use base_d::{AlphabetsConfig, Alphabet, encode, decode};

fn main() {
    let config = AlphabetsConfig::load_default().unwrap();
    let alphabet_config = config.get_alphabet("cards").expect("cards alphabet not found");
    let chars: Vec<char> = alphabet_config.chars.chars().collect();
    let padding = alphabet_config.padding.as_ref().and_then(|s| s.chars().next());
    let alphabet = Alphabet::new_with_mode(chars, alphabet_config.mode.clone(), padding).unwrap();
    
    let data = b"Hello, World!";
    
    println!("Original: {}", String::from_utf8_lossy(data));
    println!("Alphabet: cards (base-{})", alphabet.base());
    let encoded = encode(data, &alphabet);
    println!("Encoded:  {}", encoded);
    
    let decoded = decode(&encoded, &alphabet).unwrap();
    println!("Decoded:  {}", String::from_utf8_lossy(&decoded));
    println!("\nRoundtrip successful: {}", data == &decoded[..]);
}
