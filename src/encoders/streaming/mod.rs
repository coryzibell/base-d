mod decoder;
mod encoder;
mod hasher;

pub use decoder::StreamingDecoder;
pub use encoder::StreamingEncoder;

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::{Dictionary, DictionaryRegistry};
    use std::io::Cursor;

    fn get_dictionary(name: &str) -> Dictionary {
        let config = DictionaryRegistry::load_default().unwrap();
        let dictionary_config = config.get_dictionary(name).unwrap();
        let effective_mode = dictionary_config.effective_mode();

        match effective_mode {
            crate::core::config::EncodingMode::ByteRange => {
                let start = dictionary_config.start_codepoint.unwrap();
                Dictionary::new_with_mode_and_range(Vec::new(), effective_mode, None, Some(start))
                    .unwrap()
            }
            _ => {
                let chars: Vec<char> = dictionary_config
                    .effective_chars()
                    .unwrap()
                    .chars()
                    .collect();
                let padding = dictionary_config
                    .padding
                    .as_ref()
                    .and_then(|s| s.chars().next());
                Dictionary::new_with_mode(chars, effective_mode, padding).unwrap()
            }
        }
    }

    #[test]
    fn test_streaming_encode_decode_base64() {
        let dictionary = get_dictionary("base64");
        let data = b"Hello, World! This is a streaming test with multiple chunks of data.";

        // Encode
        let mut encoded_output = Vec::new();
        {
            let mut encoder = StreamingEncoder::new(&dictionary, &mut encoded_output);
            let mut reader = Cursor::new(data);
            encoder.encode(&mut reader).unwrap();
        }

        // Decode
        let mut decoded_output = Vec::new();
        {
            let mut decoder = StreamingDecoder::new(&dictionary, &mut decoded_output);
            let mut reader = Cursor::new(&encoded_output);
            decoder.decode(&mut reader).unwrap();
        }

        assert_eq!(data, &decoded_output[..]);
    }

    #[test]
    fn test_streaming_encode_decode_base100() {
        let dictionary = get_dictionary("base100");
        let data = b"Test data for byte range streaming";

        // Encode
        let mut encoded_output = Vec::new();
        {
            let mut encoder = StreamingEncoder::new(&dictionary, &mut encoded_output);
            let mut reader = Cursor::new(data);
            encoder.encode(&mut reader).unwrap();
        }

        // Decode
        let mut decoded_output = Vec::new();
        {
            let mut decoder = StreamingDecoder::new(&dictionary, &mut decoded_output);
            let mut reader = Cursor::new(&encoded_output);
            decoder.decode(&mut reader).unwrap();
        }

        assert_eq!(data, &decoded_output[..]);
    }

    #[test]
    fn test_streaming_large_data() {
        let dictionary = get_dictionary("base64");
        // Create 100KB of data
        let data: Vec<u8> = (0..100000).map(|i| (i % 256) as u8).collect();

        // Encode
        let mut encoded_output = Vec::new();
        {
            let mut encoder = StreamingEncoder::new(&dictionary, &mut encoded_output);
            let mut reader = Cursor::new(&data);
            encoder.encode(&mut reader).unwrap();
        }

        // Decode
        let mut decoded_output = Vec::new();
        {
            let mut decoder = StreamingDecoder::new(&dictionary, &mut decoded_output);
            let mut reader = Cursor::new(&encoded_output);
            decoder.decode(&mut reader).unwrap();
        }

        assert_eq!(data, decoded_output);
    }
}
