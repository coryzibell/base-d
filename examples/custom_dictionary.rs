use base_d::{Dictionary, decode, encode};

fn main() {
    // Create a custom dictionary with just 4 DNA bases
    let dna_dictionary = Dictionary::builder()
        .chars_from_str("ACGT")
        .build()
        .unwrap();

    println!("DNA Dictionary (base-4)");
    println!("=====================\n");

    // Encode some data
    let data = b"DNA";
    let encoded = encode(data, &dna_dictionary);

    println!("Original: {:?}", String::from_utf8_lossy(data));
    println!("Encoded:  {}", encoded);
    println!("Length:   {} bases\n", encoded.len());

    // Decode it back
    let decoded = decode(&encoded, &dna_dictionary).unwrap();
    println!("Decoded:  {:?}", String::from_utf8_lossy(&decoded));
    println!("Match:    {}", data == &decoded[..]);

    // Try different data
    println!("\n---\n");
    let data2 = &[0xFF, 0x00, 0x42];
    let encoded2 = encode(data2, &dna_dictionary);
    println!("Binary {:?} encodes to: {}", data2, encoded2);
}
