use base_d::{Alphabet, encode, decode};

fn main() {
    // Create a custom alphabet with just 4 DNA bases
    let dna_alphabet = Alphabet::from_str("ACGT").unwrap();
    
    println!("DNA Alphabet (base-4)");
    println!("=====================\n");
    
    // Encode some data
    let data = b"DNA";
    let encoded = encode(data, &dna_alphabet);
    
    println!("Original: {:?}", String::from_utf8_lossy(data));
    println!("Encoded:  {}", encoded);
    println!("Length:   {} bases\n", encoded.len());
    
    // Decode it back
    let decoded = decode(&encoded, &dna_alphabet).unwrap();
    println!("Decoded:  {:?}", String::from_utf8_lossy(&decoded));
    println!("Match:    {}", data == &decoded[..]);
    
    // Try different data
    println!("\n---\n");
    let data2 = &[0xFF, 0x00, 0x42];
    let encoded2 = encode(data2, &dna_alphabet);
    println!("Binary {:?} encodes to: {}", data2, encoded2);
}
