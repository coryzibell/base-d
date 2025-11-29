use crate::core::dictionary::Dictionary;
use crate::encoders::algorithms::DecodeError;
use crate::features::compression::CompressionAlgorithm;
use crate::features::hashing::HashAlgorithm;
use std::io::{Read, Write};

use super::hasher::{create_hasher_writer, HasherWriter};

const CHUNK_SIZE: usize = 4096; // 4KB chunks

/// Streaming decoder for processing large amounts of encoded data efficiently.
///
/// Processes data in chunks to avoid loading entire files into memory.
/// Suitable for decoding large files or network streams.
/// Supports optional decompression and hashing during decoding.
pub struct StreamingDecoder<'a, W: Write> {
    dictionary: &'a Dictionary,
    writer: W,
    decompress_algo: Option<CompressionAlgorithm>,
    hash_algo: Option<HashAlgorithm>,
    xxhash_config: crate::features::hashing::XxHashConfig,
}

impl<'a, W: Write> StreamingDecoder<'a, W> {
    /// Creates a new streaming decoder.
    ///
    /// # Arguments
    ///
    /// * `dictionary` - The dictionary used for encoding
    /// * `writer` - The destination for decoded output
    pub fn new(dictionary: &'a Dictionary, writer: W) -> Self {
        StreamingDecoder {
            dictionary,
            writer,
            decompress_algo: None,
            hash_algo: None,
            xxhash_config: crate::features::hashing::XxHashConfig::default(),
        }
    }

    /// Sets decompression algorithm.
    pub fn with_decompression(mut self, algo: CompressionAlgorithm) -> Self {
        self.decompress_algo = Some(algo);
        self
    }

    /// Sets hash algorithm for computing hash during decoding.
    pub fn with_hashing(mut self, algo: HashAlgorithm) -> Self {
        self.hash_algo = Some(algo);
        self
    }

    /// Sets xxHash configuration (seed and secret).
    pub fn with_xxhash_config(mut self, config: crate::features::hashing::XxHashConfig) -> Self {
        self.xxhash_config = config;
        self
    }

    /// Decodes data from a reader in chunks.
    ///
    /// Note: BaseConversion mode requires reading the entire input at once
    /// due to the mathematical nature of the algorithm. For truly streaming
    /// behavior, use Chunked or ByteRange modes.
    ///
    /// Returns the computed hash if hash_algo was set, otherwise None.
    pub fn decode<R: Read>(&mut self, reader: &mut R) -> Result<Option<Vec<u8>>, DecodeError> {
        // If decompression is enabled, decode then decompress
        if let Some(algo) = self.decompress_algo {
            return self.decode_with_decompression(reader, algo);
        }

        // No decompression - decode directly with optional hashing
        match self.dictionary.mode() {
            crate::core::config::EncodingMode::Chunked => self.decode_chunked(reader),
            crate::core::config::EncodingMode::ByteRange => self.decode_byte_range(reader),
            crate::core::config::EncodingMode::BaseConversion => {
                // Mathematical mode requires entire input
                let mut buffer = String::new();
                reader
                    .read_to_string(&mut buffer)
                    .map_err(|_| DecodeError::InvalidCharacter {
                        char: '\0',
                        position: 0,
                        input: String::new(),
                        valid_chars: String::new(),
                    })?;
                let decoded = crate::encoders::algorithms::math::decode(&buffer, self.dictionary)?;

                let hash = self
                    .hash_algo
                    .map(|algo| crate::features::hashing::hash(&decoded, algo));

                self.writer
                    .write_all(&decoded)
                    .map_err(|_| DecodeError::InvalidCharacter {
                        char: '\0',
                        position: 0,
                        input: String::new(),
                        valid_chars: String::new(),
                    })?;
                Ok(hash)
            }
        }
    }

    /// Decode with decompression: decode stream then decompress decoded data.
    fn decode_with_decompression<R: Read>(
        &mut self,
        reader: &mut R,
        algo: CompressionAlgorithm,
    ) -> Result<Option<Vec<u8>>, DecodeError> {
        use std::io::Cursor;

        // Decode the input stream to get compressed data
        let mut compressed_data = Vec::new();
        {
            let mut temp_decoder = StreamingDecoder::new(self.dictionary, &mut compressed_data);
            temp_decoder.decode(reader)?;
        }

        // Decompress and write to output with optional hashing
        let mut cursor = Cursor::new(compressed_data);
        let hash = self.decompress_stream(&mut cursor, algo).map_err(|_| {
            DecodeError::InvalidCharacter {
                char: '\0',
                position: 0,
                input: String::new(),
                valid_chars: String::new(),
            }
        })?;

        Ok(hash)
    }

    /// Decompress a stream with optional hashing.
    fn decompress_stream<R: Read>(
        &mut self,
        reader: &mut R,
        algo: CompressionAlgorithm,
    ) -> std::io::Result<Option<Vec<u8>>> {
        use flate2::read::GzDecoder;
        use xz2::read::XzDecoder;

        let mut hasher = self
            .hash_algo
            .map(|algo| create_hasher_writer(algo, &self.xxhash_config));

        match algo {
            CompressionAlgorithm::Gzip => {
                let mut decoder = GzDecoder::new(reader);
                Self::copy_with_hash_to_writer(&mut decoder, &mut self.writer, &mut hasher)?;
            }
            CompressionAlgorithm::Zstd => {
                let mut decoder = zstd::stream::read::Decoder::new(reader)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                Self::copy_with_hash_to_writer(&mut decoder, &mut self.writer, &mut hasher)?;
            }
            CompressionAlgorithm::Brotli => {
                let mut decoder = brotli::Decompressor::new(reader, 4096);
                Self::copy_with_hash_to_writer(&mut decoder, &mut self.writer, &mut hasher)?;
            }
            CompressionAlgorithm::Lzma => {
                let mut decoder = XzDecoder::new(reader);
                Self::copy_with_hash_to_writer(&mut decoder, &mut self.writer, &mut hasher)?;
            }
            CompressionAlgorithm::Lz4 | CompressionAlgorithm::Snappy => {
                // LZ4 and Snappy don't have streaming decoders
                let mut compressed = Vec::new();
                reader.read_to_end(&mut compressed)?;

                let decompressed = match algo {
                    CompressionAlgorithm::Lz4 => {
                        lz4::block::decompress(&compressed, Some(100 * 1024 * 1024))
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                    }
                    CompressionAlgorithm::Snappy => {
                        let mut decoder = snap::raw::Decoder::new();
                        decoder
                            .decompress_vec(&compressed)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                    }
                    _ => unreachable!(),
                };

                let hash = self
                    .hash_algo
                    .map(|algo| crate::features::hashing::hash(&decompressed, algo));
                self.writer.write_all(&decompressed)?;
                return Ok(hash);
            }
        }

        Ok(hasher.map(|h| h.finalize()))
    }

    fn copy_with_hash_to_writer<R: Read>(
        reader: &mut R,
        writer: &mut W,
        hasher: &mut Option<HasherWriter>,
    ) -> std::io::Result<()> {
        let mut buffer = vec![0u8; CHUNK_SIZE];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let chunk = &buffer[..bytes_read];
            if let Some(ref mut h) = hasher {
                h.update(chunk);
            }
            writer.write_all(chunk)?;
        }

        Ok(())
    }

    fn decode_chunked<R: Read>(&mut self, reader: &mut R) -> Result<Option<Vec<u8>>, DecodeError> {
        let base = self.dictionary.base();
        let bits_per_char = (base as f64).log2() as usize;
        let chars_per_group = 8 / bits_per_char;

        // Read text in chunks
        let mut text_buffer = String::new();
        let mut char_buffer = vec![0u8; CHUNK_SIZE];
        let mut hasher = self
            .hash_algo
            .map(|algo| create_hasher_writer(algo, &self.xxhash_config));

        loop {
            let bytes_read =
                reader
                    .read(&mut char_buffer)
                    .map_err(|_| DecodeError::InvalidCharacter {
                        char: '\0',
                        position: 0,
                        input: String::new(),
                        valid_chars: String::new(),
                    })?;
            if bytes_read == 0 {
                break;
            }

            let chunk_str = std::str::from_utf8(&char_buffer[..bytes_read]).map_err(|_| {
                DecodeError::InvalidCharacter {
                    char: '\0',
                    position: 0,
                    input: String::new(),
                    valid_chars: String::new(),
                }
            })?;
            text_buffer.push_str(chunk_str);

            // Process complete character groups
            let chars: Vec<char> = text_buffer.chars().collect();
            let complete_groups = (chars.len() / chars_per_group) * chars_per_group;

            if complete_groups > 0 {
                let to_decode: String = chars[..complete_groups].iter().collect();
                let decoded = crate::encoders::algorithms::chunked::decode_chunked(
                    &to_decode,
                    self.dictionary,
                )?;

                if let Some(ref mut h) = hasher {
                    h.update(&decoded);
                }

                self.writer
                    .write_all(&decoded)
                    .map_err(|_| DecodeError::InvalidCharacter {
                        char: '\0',
                        position: 0,
                        input: String::new(),
                        valid_chars: String::new(),
                    })?;

                // Keep remaining chars for next iteration
                text_buffer = chars[complete_groups..].iter().collect();
            }
        }

        // Process any remaining characters
        if !text_buffer.is_empty() {
            let decoded = crate::encoders::algorithms::chunked::decode_chunked(
                &text_buffer,
                self.dictionary,
            )?;

            if let Some(ref mut h) = hasher {
                h.update(&decoded);
            }

            self.writer
                .write_all(&decoded)
                .map_err(|_| DecodeError::InvalidCharacter {
                    char: '\0',
                    position: 0,
                    input: String::new(),
                    valid_chars: String::new(),
                })?;
        }

        Ok(hasher.map(|h| h.finalize()))
    }

    fn decode_byte_range<R: Read>(
        &mut self,
        reader: &mut R,
    ) -> Result<Option<Vec<u8>>, DecodeError> {
        let mut char_buffer = vec![0u8; CHUNK_SIZE];
        let mut hasher = self
            .hash_algo
            .map(|algo| create_hasher_writer(algo, &self.xxhash_config));

        loop {
            let bytes_read =
                reader
                    .read(&mut char_buffer)
                    .map_err(|_| DecodeError::InvalidCharacter {
                        char: '\0',
                        position: 0,
                        input: String::new(),
                        valid_chars: String::new(),
                    })?;
            if bytes_read == 0 {
                break;
            }

            let chunk_str = std::str::from_utf8(&char_buffer[..bytes_read]).map_err(|_| {
                DecodeError::InvalidCharacter {
                    char: '\0',
                    position: 0,
                    input: String::new(),
                    valid_chars: String::new(),
                }
            })?;

            let decoded = crate::encoders::algorithms::byte_range::decode_byte_range(
                chunk_str,
                self.dictionary,
            )?;

            if let Some(ref mut h) = hasher {
                h.update(&decoded);
            }

            self.writer
                .write_all(&decoded)
                .map_err(|_| DecodeError::InvalidCharacter {
                    char: '\0',
                    position: 0,
                    input: String::new(),
                    valid_chars: String::new(),
                })?;
        }

        Ok(hasher.map(|h| h.finalize()))
    }
}
