use crate::core::dictionary::Dictionary;
use crate::features::compression::CompressionAlgorithm;
use crate::features::hashing::HashAlgorithm;
use std::io::{Read, Write};

use super::hasher::{HasherWriter, create_hasher_writer};

const CHUNK_SIZE: usize = 4096; // 4KB chunks

/// Streaming encoder for processing large amounts of data efficiently.
///
/// Processes data in chunks to avoid loading entire files into memory.
/// Suitable for encoding large files or network streams.
/// Supports optional compression and hashing during encoding.
pub struct StreamingEncoder<'a, W: Write> {
    dictionary: &'a Dictionary,
    writer: W,
    compress_algo: Option<CompressionAlgorithm>,
    compress_level: u32,
    hash_algo: Option<HashAlgorithm>,
    xxhash_config: crate::features::hashing::XxHashConfig,
}

impl<'a, W: Write> StreamingEncoder<'a, W> {
    /// Creates a new streaming encoder.
    ///
    /// # Arguments
    ///
    /// * `dictionary` - The dictionary to use for encoding
    /// * `writer` - The destination for encoded output
    pub fn new(dictionary: &'a Dictionary, writer: W) -> Self {
        StreamingEncoder {
            dictionary,
            writer,
            compress_algo: None,
            compress_level: 6,
            hash_algo: None,
            xxhash_config: crate::features::hashing::XxHashConfig::default(),
        }
    }

    /// Sets compression algorithm and level.
    pub fn with_compression(mut self, algo: CompressionAlgorithm, level: u32) -> Self {
        self.compress_algo = Some(algo);
        self.compress_level = level;
        self
    }

    /// Sets hash algorithm for computing hash during encoding.
    pub fn with_hashing(mut self, algo: HashAlgorithm) -> Self {
        self.hash_algo = Some(algo);
        self
    }

    /// Sets xxHash configuration (seed and secret).
    pub fn with_xxhash_config(mut self, config: crate::features::hashing::XxHashConfig) -> Self {
        self.xxhash_config = config;
        self
    }

    /// Encodes data from a reader in chunks.
    ///
    /// Note: BaseConversion mode requires reading the entire input at once
    /// due to the mathematical nature of the algorithm. For truly streaming
    /// behavior, use Chunked or ByteRange modes.
    ///
    /// Returns the computed hash if hash_algo was set, otherwise None.
    pub fn encode<R: Read>(&mut self, reader: &mut R) -> std::io::Result<Option<Vec<u8>>> {
        // If compression is enabled, we need to compress then encode
        if let Some(algo) = self.compress_algo {
            return self.encode_with_compression(reader, algo);
        }

        // No compression - encode directly with optional hashing
        let hash = match self.dictionary.mode() {
            crate::core::config::EncodingMode::Chunked => self.encode_chunked(reader)?,
            crate::core::config::EncodingMode::ByteRange => self.encode_byte_range(reader)?,
            crate::core::config::EncodingMode::BaseConversion => {
                // Mathematical mode requires entire input - read all and encode
                let mut buffer = Vec::new();
                reader.read_to_end(&mut buffer)?;

                let hash = self
                    .hash_algo
                    .map(|algo| crate::features::hashing::hash(&buffer, algo));

                let encoded = crate::encoders::algorithms::math::encode(&buffer, self.dictionary);
                self.writer.write_all(encoded.as_bytes())?;
                hash
            }
        };

        Ok(hash)
    }

    /// Encode with compression: compress stream then encode compressed data.
    fn encode_with_compression<R: Read>(
        &mut self,
        reader: &mut R,
        algo: CompressionAlgorithm,
    ) -> std::io::Result<Option<Vec<u8>>> {
        use std::io::Cursor;

        // Compress the input stream
        let mut compressed_data = Vec::new();
        let hash = self.compress_stream(reader, &mut compressed_data, algo)?;

        // Encode the compressed data
        let mut cursor = Cursor::new(compressed_data);
        match self.dictionary.mode() {
            crate::core::config::EncodingMode::Chunked => {
                self.encode_chunked_no_hash(&mut cursor)?;
            }
            crate::core::config::EncodingMode::ByteRange => {
                self.encode_byte_range_no_hash(&mut cursor)?;
            }
            crate::core::config::EncodingMode::BaseConversion => {
                let buffer = cursor.into_inner();
                let encoded = crate::encoders::algorithms::math::encode(&buffer, self.dictionary);
                self.writer.write_all(encoded.as_bytes())?;
            }
        }

        Ok(hash)
    }

    /// Compress a stream with optional hashing.
    fn compress_stream<R: Read>(
        &mut self,
        reader: &mut R,
        output: &mut Vec<u8>,
        algo: CompressionAlgorithm,
    ) -> std::io::Result<Option<Vec<u8>>> {
        use flate2::write::GzEncoder;
        use xz2::write::XzEncoder;

        let hasher = self
            .hash_algo
            .map(|algo| create_hasher_writer(algo, &self.xxhash_config));

        match algo {
            CompressionAlgorithm::Gzip => {
                let mut encoder =
                    GzEncoder::new(output, flate2::Compression::new(self.compress_level));
                let hash = Self::copy_with_hash(reader, &mut encoder, hasher)?;
                encoder.finish()?;
                Ok(hash)
            }
            CompressionAlgorithm::Zstd => {
                let mut encoder =
                    zstd::stream::write::Encoder::new(output, self.compress_level as i32)
                        .map_err(std::io::Error::other)?;
                let hash = Self::copy_with_hash(reader, &mut encoder, hasher)?;
                encoder.finish()?;
                Ok(hash)
            }
            CompressionAlgorithm::Brotli => {
                let mut encoder =
                    brotli::CompressorWriter::new(output, 4096, self.compress_level, 22);
                let hash = Self::copy_with_hash(reader, &mut encoder, hasher)?;
                Ok(hash)
            }
            CompressionAlgorithm::Lzma => {
                let mut encoder = XzEncoder::new(output, self.compress_level);
                let hash = Self::copy_with_hash(reader, &mut encoder, hasher)?;
                encoder.finish()?;
                Ok(hash)
            }
            CompressionAlgorithm::Lz4 | CompressionAlgorithm::Snappy => {
                // LZ4 and Snappy don't have streaming encoders in their crates
                // Read all, compress, write
                let mut buffer = Vec::new();
                reader.read_to_end(&mut buffer)?;

                let hash = self
                    .hash_algo
                    .map(|algo| crate::features::hashing::hash(&buffer, algo));

                let compressed = match algo {
                    CompressionAlgorithm::Lz4 => lz4::block::compress(&buffer, None, false)
                        .map_err(std::io::Error::other)?,
                    CompressionAlgorithm::Snappy => {
                        let mut encoder = snap::raw::Encoder::new();
                        encoder
                            .compress_vec(&buffer)
                            .map_err(std::io::Error::other)?
                    }
                    _ => unreachable!(),
                };
                output.extend_from_slice(&compressed);
                Ok(hash)
            }
        }
    }

    fn copy_with_hash<R: Read>(
        reader: &mut R,
        writer: &mut impl Write,
        mut hasher: Option<HasherWriter>,
    ) -> std::io::Result<Option<Vec<u8>>> {
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

        Ok(hasher.map(|h| h.finalize()))
    }

    fn encode_chunked<R: Read>(&mut self, reader: &mut R) -> std::io::Result<Option<Vec<u8>>> {
        let base = self.dictionary.base();
        let bits_per_char = (base as f64).log2() as usize;
        let bytes_per_group = bits_per_char;

        // Adjust chunk size to align with encoding groups
        let aligned_chunk_size = (CHUNK_SIZE / bytes_per_group) * bytes_per_group;
        let mut buffer = vec![0u8; aligned_chunk_size];

        let mut hasher = self
            .hash_algo
            .map(|algo| create_hasher_writer(algo, &self.xxhash_config));

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let chunk = &buffer[..bytes_read];
            if let Some(ref mut h) = hasher {
                h.update(chunk);
            }

            let encoded =
                crate::encoders::algorithms::chunked::encode_chunked(chunk, self.dictionary);
            self.writer.write_all(encoded.as_bytes())?;
        }

        Ok(hasher.map(|h| h.finalize()))
    }

    fn encode_chunked_no_hash<R: Read>(&mut self, reader: &mut R) -> std::io::Result<()> {
        let base = self.dictionary.base();
        let bits_per_char = (base as f64).log2() as usize;
        let bytes_per_group = bits_per_char;

        let aligned_chunk_size = (CHUNK_SIZE / bytes_per_group) * bytes_per_group;
        let mut buffer = vec![0u8; aligned_chunk_size];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let encoded = crate::encoders::algorithms::chunked::encode_chunked(
                &buffer[..bytes_read],
                self.dictionary,
            );
            self.writer.write_all(encoded.as_bytes())?;
        }

        Ok(())
    }

    fn encode_byte_range<R: Read>(&mut self, reader: &mut R) -> std::io::Result<Option<Vec<u8>>> {
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut hasher = self
            .hash_algo
            .map(|algo| create_hasher_writer(algo, &self.xxhash_config));

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let chunk = &buffer[..bytes_read];
            if let Some(ref mut h) = hasher {
                h.update(chunk);
            }

            let encoded =
                crate::encoders::algorithms::byte_range::encode_byte_range(chunk, self.dictionary);
            self.writer.write_all(encoded.as_bytes())?;
        }

        Ok(hasher.map(|h| h.finalize()))
    }

    fn encode_byte_range_no_hash<R: Read>(&mut self, reader: &mut R) -> std::io::Result<()> {
        let mut buffer = vec![0u8; CHUNK_SIZE];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let encoded = crate::encoders::algorithms::byte_range::encode_byte_range(
                &buffer[..bytes_read],
                self.dictionary,
            );
            self.writer.write_all(encoded.as_bytes())?;
        }

        Ok(())
    }
}
