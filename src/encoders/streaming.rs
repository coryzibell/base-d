use crate::core::alphabet::Alphabet;
use crate::encoders::encoding::DecodeError;
use std::io::{Read, Write};

const CHUNK_SIZE: usize = 4096; // 4KB chunks

/// Streaming encoder for processing large amounts of data efficiently.
///
/// Processes data in chunks to avoid loading entire files into memory.
/// Suitable for encoding large files or network streams.
pub struct StreamingEncoder<'a, W: Write> {
    alphabet: &'a Alphabet,
    writer: W,
}

impl<'a, W: Write> StreamingEncoder<'a, W> {
    /// Creates a new streaming encoder.
    ///
    /// # Arguments
    ///
    /// * `alphabet` - The alphabet to use for encoding
    /// * `writer` - The destination for encoded output
    pub fn new(alphabet: &'a Alphabet, writer: W) -> Self {
        StreamingEncoder { alphabet, writer }
    }
    
    /// Encodes data from a reader in chunks.
    ///
    /// Note: BaseConversion mode requires reading the entire input at once
    /// due to the mathematical nature of the algorithm. For truly streaming
    /// behavior, use Chunked or ByteRange modes.
    pub fn encode<R: Read>(&mut self, reader: &mut R) -> std::io::Result<()> {
        match self.alphabet.mode() {
            crate::core::config::EncodingMode::Chunked => {
                self.encode_chunked(reader)
            }
            crate::core::config::EncodingMode::ByteRange => {
                self.encode_byte_range(reader)
            }
            crate::core::config::EncodingMode::BaseConversion => {
                // Mathematical mode requires entire input - read all and encode
                let mut buffer = Vec::new();
                reader.read_to_end(&mut buffer)?;
                let encoded = crate::encoders::encoding::encode(&buffer, self.alphabet);
                self.writer.write_all(encoded.as_bytes())?;
                Ok(())
            }
        }
    }
    
    fn encode_chunked<R: Read>(&mut self, reader: &mut R) -> std::io::Result<()> {
        let base = self.alphabet.base();
        let bits_per_char = (base as f64).log2() as usize;
        let bytes_per_group = bits_per_char;
        
        // Adjust chunk size to align with encoding groups
        let aligned_chunk_size = (CHUNK_SIZE / bytes_per_group) * bytes_per_group;
        let mut buffer = vec![0u8; aligned_chunk_size];
        
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            
            let encoded = crate::encoders::chunked::encode_chunked(&buffer[..bytes_read], self.alphabet);
            self.writer.write_all(encoded.as_bytes())?;
        }
        
        Ok(())
    }
    
    fn encode_byte_range<R: Read>(&mut self, reader: &mut R) -> std::io::Result<()> {
        let mut buffer = vec![0u8; CHUNK_SIZE];
        
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            
            let encoded = crate::encoders::byte_range::encode_byte_range(&buffer[..bytes_read], self.alphabet);
            self.writer.write_all(encoded.as_bytes())?;
        }
        
        Ok(())
    }
}

/// Streaming decoder for processing large amounts of encoded data efficiently.
///
/// Processes data in chunks to avoid loading entire files into memory.
/// Suitable for decoding large files or network streams.
pub struct StreamingDecoder<'a, W: Write> {
    alphabet: &'a Alphabet,
    writer: W,
}

impl<'a, W: Write> StreamingDecoder<'a, W> {
    /// Creates a new streaming decoder.
    ///
    /// # Arguments
    ///
    /// * `alphabet` - The alphabet used for encoding
    /// * `writer` - The destination for decoded output
    pub fn new(alphabet: &'a Alphabet, writer: W) -> Self {
        StreamingDecoder { alphabet, writer }
    }
    
    /// Decodes data from a reader in chunks.
    ///
    /// Note: BaseConversion mode requires reading the entire input at once
    /// due to the mathematical nature of the algorithm. For truly streaming
    /// behavior, use Chunked or ByteRange modes.
    pub fn decode<R: Read>(&mut self, reader: &mut R) -> Result<(), DecodeError> {
        match self.alphabet.mode() {
            crate::core::config::EncodingMode::Chunked => {
                self.decode_chunked(reader)
            }
            crate::core::config::EncodingMode::ByteRange => {
                self.decode_byte_range(reader)
            }
            crate::core::config::EncodingMode::BaseConversion => {
                // Mathematical mode requires entire input
                let mut buffer = String::new();
                reader.read_to_string(&mut buffer)
                    .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
                let decoded = crate::encoders::encoding::decode(&buffer, self.alphabet)?;
                self.writer.write_all(&decoded)
                    .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
                Ok(())
            }
        }
    }
    
    fn decode_chunked<R: Read>(&mut self, reader: &mut R) -> Result<(), DecodeError> {
        let base = self.alphabet.base();
        let bits_per_char = (base as f64).log2() as usize;
        let chars_per_group = 8 / bits_per_char;
        
        // Read text in chunks
        let mut text_buffer = String::new();
        let mut char_buffer = vec![0u8; CHUNK_SIZE];
        
        loop {
            let bytes_read = reader.read(&mut char_buffer)
                .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
            if bytes_read == 0 {
                break;
            }
            
            let chunk_str = std::str::from_utf8(&char_buffer[..bytes_read])
                .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
            text_buffer.push_str(chunk_str);
            
            // Process complete character groups
            let chars: Vec<char> = text_buffer.chars().collect();
            let complete_groups = (chars.len() / chars_per_group) * chars_per_group;
            
            if complete_groups > 0 {
                let to_decode: String = chars[..complete_groups].iter().collect();
                let decoded = crate::encoders::chunked::decode_chunked(&to_decode, self.alphabet)?;
                self.writer.write_all(&decoded)
                    .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
                
                // Keep remaining chars for next iteration
                text_buffer = chars[complete_groups..].iter().collect();
            }
        }
        
        // Process any remaining characters
        if !text_buffer.is_empty() {
            let decoded = crate::encoders::chunked::decode_chunked(&text_buffer, self.alphabet)?;
            self.writer.write_all(&decoded)
                .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
        }
        
        Ok(())
    }
    
    fn decode_byte_range<R: Read>(&mut self, reader: &mut R) -> Result<(), DecodeError> {
        let mut char_buffer = vec![0u8; CHUNK_SIZE];
        
        loop {
            let bytes_read = reader.read(&mut char_buffer)
                .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
            if bytes_read == 0 {
                break;
            }
            
            let chunk_str = std::str::from_utf8(&char_buffer[..bytes_read])
                .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
            
            let decoded = crate::encoders::byte_range::decode_byte_range(chunk_str, self.alphabet)?;
            self.writer.write_all(&decoded)
                .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AlphabetsConfig, Alphabet};
    use std::io::Cursor;
    
    fn get_alphabet(name: &str) -> Alphabet {
        let config = AlphabetsConfig::load_default().unwrap();
        let alphabet_config = config.get_alphabet(name).unwrap();
        
        match alphabet_config.mode {
            crate::core::config::EncodingMode::ByteRange => {
                let start = alphabet_config.start_codepoint.unwrap();
                Alphabet::new_with_mode_and_range(Vec::new(), alphabet_config.mode.clone(), None, Some(start)).unwrap()
            }
            _ => {
                let chars: Vec<char> = alphabet_config.chars.chars().collect();
                let padding = alphabet_config.padding.as_ref().and_then(|s| s.chars().next());
                Alphabet::new_with_mode(chars, alphabet_config.mode.clone(), padding).unwrap()
            }
        }
    }
    
    #[test]
    fn test_streaming_encode_decode_base64() {
        let alphabet = get_alphabet("base64");
        let data = b"Hello, World! This is a streaming test with multiple chunks of data.";
        
        // Encode
        let mut encoded_output = Vec::new();
        {
            let mut encoder = StreamingEncoder::new(&alphabet, &mut encoded_output);
            let mut reader = Cursor::new(data);
            encoder.encode(&mut reader).unwrap();
        }
        
        // Decode
        let mut decoded_output = Vec::new();
        {
            let mut decoder = StreamingDecoder::new(&alphabet, &mut decoded_output);
            let mut reader = Cursor::new(&encoded_output);
            decoder.decode(&mut reader).unwrap();
        }
        
        assert_eq!(data, &decoded_output[..]);
    }
    
    #[test]
    fn test_streaming_encode_decode_base100() {
        let alphabet = get_alphabet("base100");
        let data = b"Test data for byte range streaming";
        
        // Encode
        let mut encoded_output = Vec::new();
        {
            let mut encoder = StreamingEncoder::new(&alphabet, &mut encoded_output);
            let mut reader = Cursor::new(data);
            encoder.encode(&mut reader).unwrap();
        }
        
        // Decode
        let mut decoded_output = Vec::new();
        {
            let mut decoder = StreamingDecoder::new(&alphabet, &mut decoded_output);
            let mut reader = Cursor::new(&encoded_output);
            decoder.decode(&mut reader).unwrap();
        }
        
        assert_eq!(data, &decoded_output[..]);
    }
    
    #[test]
    fn test_streaming_large_data() {
        let alphabet = get_alphabet("base64");
        // Create 100KB of data
        let data: Vec<u8> = (0..100000).map(|i| (i % 256) as u8).collect();
        
        // Encode
        let mut encoded_output = Vec::new();
        {
            let mut encoder = StreamingEncoder::new(&alphabet, &mut encoded_output);
            let mut reader = Cursor::new(&data);
            encoder.encode(&mut reader).unwrap();
        }
        
        // Decode
        let mut decoded_output = Vec::new();
        {
            let mut decoder = StreamingDecoder::new(&alphabet, &mut decoded_output);
            let mut reader = Cursor::new(&encoded_output);
            decoder.decode(&mut reader).unwrap();
        }
        
        assert_eq!(data, decoded_output);
    }
}
