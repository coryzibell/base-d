use crate::compression::CompressionAlgorithm;
use crate::core::dictionary::Dictionary;
use crate::encoders::math::DecodeError;
use crate::hashing::HashAlgorithm;
use std::io::{Read, Write};

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
    xxhash_config: crate::hashing::XxHashConfig,
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
            xxhash_config: crate::hashing::XxHashConfig::default(),
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
    pub fn with_xxhash_config(mut self, config: crate::hashing::XxHashConfig) -> Self {
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
                    .map(|algo| crate::hashing::hash(&buffer, algo));

                let encoded = crate::encoders::math::encode(&buffer, self.dictionary);
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
                let encoded = crate::encoders::math::encode(&buffer, self.dictionary);
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
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
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
                    .map(|algo| crate::hashing::hash(&buffer, algo));

                let compressed = match algo {
                    CompressionAlgorithm::Lz4 => lz4::block::compress(&buffer, None, false)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?,
                    CompressionAlgorithm::Snappy => {
                        let mut encoder = snap::raw::Encoder::new();
                        encoder
                            .compress_vec(&buffer)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
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

            let encoded = crate::encoders::chunked::encode_chunked(chunk, self.dictionary);
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

            let encoded =
                crate::encoders::chunked::encode_chunked(&buffer[..bytes_read], self.dictionary);
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

            let encoded = crate::encoders::byte_range::encode_byte_range(chunk, self.dictionary);
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

            let encoded = crate::encoders::byte_range::encode_byte_range(
                &buffer[..bytes_read],
                self.dictionary,
            );
            self.writer.write_all(encoded.as_bytes())?;
        }

        Ok(())
    }
}

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
    xxhash_config: crate::hashing::XxHashConfig,
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
            xxhash_config: crate::hashing::XxHashConfig::default(),
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
    pub fn with_xxhash_config(mut self, config: crate::hashing::XxHashConfig) -> Self {
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
                    .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
                let decoded = crate::encoders::math::decode(&buffer, self.dictionary)?;

                let hash = self
                    .hash_algo
                    .map(|algo| crate::hashing::hash(&decoded, algo));

                self.writer
                    .write_all(&decoded)
                    .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
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
        let hash = self
            .decompress_stream(&mut cursor, algo)
            .map_err(|_| DecodeError::InvalidCharacter('\0'))?;

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
                    .map(|algo| crate::hashing::hash(&decompressed, algo));
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
            let bytes_read = reader
                .read(&mut char_buffer)
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
                let decoded =
                    crate::encoders::chunked::decode_chunked(&to_decode, self.dictionary)?;

                if let Some(ref mut h) = hasher {
                    h.update(&decoded);
                }

                self.writer
                    .write_all(&decoded)
                    .map_err(|_| DecodeError::InvalidCharacter('\0'))?;

                // Keep remaining chars for next iteration
                text_buffer = chars[complete_groups..].iter().collect();
            }
        }

        // Process any remaining characters
        if !text_buffer.is_empty() {
            let decoded = crate::encoders::chunked::decode_chunked(&text_buffer, self.dictionary)?;

            if let Some(ref mut h) = hasher {
                h.update(&decoded);
            }

            self.writer
                .write_all(&decoded)
                .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
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
            let bytes_read = reader
                .read(&mut char_buffer)
                .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
            if bytes_read == 0 {
                break;
            }

            let chunk_str = std::str::from_utf8(&char_buffer[..bytes_read])
                .map_err(|_| DecodeError::InvalidCharacter('\0'))?;

            let decoded =
                crate::encoders::byte_range::decode_byte_range(chunk_str, self.dictionary)?;

            if let Some(ref mut h) = hasher {
                h.update(&decoded);
            }

            self.writer
                .write_all(&decoded)
                .map_err(|_| DecodeError::InvalidCharacter('\0'))?;
        }

        Ok(hasher.map(|h| h.finalize()))
    }
}

// Helper for managing hash state during streaming
enum HasherWriter {
    Md5(md5::Md5),
    Sha224(sha2::Sha224),
    Sha256(sha2::Sha256),
    Sha384(sha2::Sha384),
    Sha512(sha2::Sha512),
    Sha3_224(sha3::Sha3_224),
    Sha3_256(sha3::Sha3_256),
    Sha3_384(sha3::Sha3_384),
    Sha3_512(sha3::Sha3_512),
    Keccak224(sha3::Keccak224),
    Keccak256(sha3::Keccak256),
    Keccak384(sha3::Keccak384),
    Keccak512(sha3::Keccak512),
    Blake2b(blake2::Blake2b512),
    Blake2s(blake2::Blake2s256),
    Blake3(blake3::Hasher),
    Crc16(Box<crc::Digest<'static, u16>>),
    Crc32(Box<crc::Digest<'static, u32>>),
    Crc32c(Box<crc::Digest<'static, u32>>),
    Crc64(Box<crc::Digest<'static, u64>>),
    XxHash32(twox_hash::XxHash32),
    XxHash64(twox_hash::XxHash64),
    XxHash3_64(twox_hash::xxhash3_64::Hasher),
    XxHash3_128(twox_hash::xxhash3_128::Hasher),
}

impl HasherWriter {
    fn update(&mut self, data: &[u8]) {
        use sha2::Digest;
        use std::hash::Hasher;

        match self {
            HasherWriter::Md5(h) => {
                h.update(data);
            }
            HasherWriter::Sha224(h) => {
                h.update(data);
            }
            HasherWriter::Sha256(h) => {
                h.update(data);
            }
            HasherWriter::Sha384(h) => {
                h.update(data);
            }
            HasherWriter::Sha512(h) => {
                h.update(data);
            }
            HasherWriter::Sha3_224(h) => {
                h.update(data);
            }
            HasherWriter::Sha3_256(h) => {
                h.update(data);
            }
            HasherWriter::Sha3_384(h) => {
                h.update(data);
            }
            HasherWriter::Sha3_512(h) => {
                h.update(data);
            }
            HasherWriter::Keccak224(h) => {
                h.update(data);
            }
            HasherWriter::Keccak256(h) => {
                h.update(data);
            }
            HasherWriter::Keccak384(h) => {
                h.update(data);
            }
            HasherWriter::Keccak512(h) => {
                h.update(data);
            }
            HasherWriter::Blake2b(h) => {
                h.update(data);
            }
            HasherWriter::Blake2s(h) => {
                h.update(data);
            }
            HasherWriter::Blake3(h) => {
                h.update(data);
            }
            HasherWriter::Crc16(digest) => {
                digest.update(data);
            }
            HasherWriter::Crc32(digest) => {
                digest.update(data);
            }
            HasherWriter::Crc32c(digest) => {
                digest.update(data);
            }
            HasherWriter::Crc64(digest) => {
                digest.update(data);
            }
            HasherWriter::XxHash32(h) => {
                h.write(data);
            }
            HasherWriter::XxHash64(h) => {
                h.write(data);
            }
            HasherWriter::XxHash3_64(h) => {
                h.write(data);
            }
            HasherWriter::XxHash3_128(h) => {
                h.write(data);
            }
        }
    }

    fn finalize(self) -> Vec<u8> {
        use sha2::Digest;
        use std::hash::Hasher;

        match self {
            HasherWriter::Md5(h) => h.finalize().to_vec(),
            HasherWriter::Sha224(h) => h.finalize().to_vec(),
            HasherWriter::Sha256(h) => h.finalize().to_vec(),
            HasherWriter::Sha384(h) => h.finalize().to_vec(),
            HasherWriter::Sha512(h) => h.finalize().to_vec(),
            HasherWriter::Sha3_224(h) => h.finalize().to_vec(),
            HasherWriter::Sha3_256(h) => h.finalize().to_vec(),
            HasherWriter::Sha3_384(h) => h.finalize().to_vec(),
            HasherWriter::Sha3_512(h) => h.finalize().to_vec(),
            HasherWriter::Keccak224(h) => h.finalize().to_vec(),
            HasherWriter::Keccak256(h) => h.finalize().to_vec(),
            HasherWriter::Keccak384(h) => h.finalize().to_vec(),
            HasherWriter::Keccak512(h) => h.finalize().to_vec(),
            HasherWriter::Blake2b(h) => h.finalize().to_vec(),
            HasherWriter::Blake2s(h) => h.finalize().to_vec(),
            HasherWriter::Blake3(h) => h.finalize().as_bytes().to_vec(),
            HasherWriter::Crc16(digest) => digest.finalize().to_be_bytes().to_vec(),
            HasherWriter::Crc32(digest) => digest.finalize().to_be_bytes().to_vec(),
            HasherWriter::Crc32c(digest) => digest.finalize().to_be_bytes().to_vec(),
            HasherWriter::Crc64(digest) => digest.finalize().to_be_bytes().to_vec(),
            HasherWriter::XxHash32(h) => (h.finish() as u32).to_be_bytes().to_vec(),
            HasherWriter::XxHash64(h) => h.finish().to_be_bytes().to_vec(),
            HasherWriter::XxHash3_64(h) => h.finish().to_be_bytes().to_vec(),
            HasherWriter::XxHash3_128(h) => {
                let hash = h.finish_128();
                let mut result = Vec::with_capacity(16);
                result.extend_from_slice(&hash.to_be_bytes());
                result
            }
        }
    }
}

fn create_hasher_writer(
    algo: HashAlgorithm,
    config: &crate::hashing::XxHashConfig,
) -> HasherWriter {
    use sha2::Digest;

    match algo {
        HashAlgorithm::Md5 => HasherWriter::Md5(md5::Md5::new()),
        HashAlgorithm::Sha224 => HasherWriter::Sha224(sha2::Sha224::new()),
        HashAlgorithm::Sha256 => HasherWriter::Sha256(sha2::Sha256::new()),
        HashAlgorithm::Sha384 => HasherWriter::Sha384(sha2::Sha384::new()),
        HashAlgorithm::Sha512 => HasherWriter::Sha512(sha2::Sha512::new()),
        HashAlgorithm::Sha3_224 => HasherWriter::Sha3_224(sha3::Sha3_224::new()),
        HashAlgorithm::Sha3_256 => HasherWriter::Sha3_256(sha3::Sha3_256::new()),
        HashAlgorithm::Sha3_384 => HasherWriter::Sha3_384(sha3::Sha3_384::new()),
        HashAlgorithm::Sha3_512 => HasherWriter::Sha3_512(sha3::Sha3_512::new()),
        HashAlgorithm::Keccak224 => HasherWriter::Keccak224(sha3::Keccak224::new()),
        HashAlgorithm::Keccak256 => HasherWriter::Keccak256(sha3::Keccak256::new()),
        HashAlgorithm::Keccak384 => HasherWriter::Keccak384(sha3::Keccak384::new()),
        HashAlgorithm::Keccak512 => HasherWriter::Keccak512(sha3::Keccak512::new()),
        HashAlgorithm::Blake2b => HasherWriter::Blake2b(blake2::Blake2b512::new()),
        HashAlgorithm::Blake2s => HasherWriter::Blake2s(blake2::Blake2s256::new()),
        HashAlgorithm::Blake3 => HasherWriter::Blake3(blake3::Hasher::new()),
        HashAlgorithm::Crc16 => {
            static CRC: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_IBM_SDLC);
            HasherWriter::Crc16(Box::new(CRC.digest()))
        }
        HashAlgorithm::Crc32 => {
            static CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
            HasherWriter::Crc32(Box::new(CRC.digest()))
        }
        HashAlgorithm::Crc32c => {
            static CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);
            HasherWriter::Crc32c(Box::new(CRC.digest()))
        }
        HashAlgorithm::Crc64 => {
            static CRC: crc::Crc<u64> = crc::Crc::<u64>::new(&crc::CRC_64_ECMA_182);
            HasherWriter::Crc64(Box::new(CRC.digest()))
        }
        HashAlgorithm::XxHash32 => {
            HasherWriter::XxHash32(twox_hash::XxHash32::with_seed(config.seed as u32))
        }
        HashAlgorithm::XxHash64 => {
            HasherWriter::XxHash64(twox_hash::XxHash64::with_seed(config.seed))
        }
        HashAlgorithm::XxHash3_64 => {
            if let Some(ref secret) = config.secret {
                HasherWriter::XxHash3_64(
                    twox_hash::xxhash3_64::Hasher::with_seed_and_secret(
                        config.seed,
                        secret.as_slice(),
                    )
                    .expect(
                        "XXH3 secret validation should have been done in XxHashConfig::with_secret",
                    ),
                )
            } else {
                HasherWriter::XxHash3_64(twox_hash::xxhash3_64::Hasher::with_seed(config.seed))
            }
        }
        HashAlgorithm::XxHash3_128 => {
            if let Some(ref secret) = config.secret {
                HasherWriter::XxHash3_128(
                    twox_hash::xxhash3_128::Hasher::with_seed_and_secret(
                        config.seed,
                        secret.as_slice(),
                    )
                    .expect(
                        "XXH3 secret validation should have been done in XxHashConfig::with_secret",
                    ),
                )
            } else {
                HasherWriter::XxHash3_128(twox_hash::xxhash3_128::Hasher::with_seed(config.seed))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DictionaryRegistry, Dictionary};
    use std::io::Cursor;

    fn get_dictionary(name: &str) -> Dictionary {
        let config = DictionaryRegistry::load_default().unwrap();
        let alphabet_config = config.get_dictionary(name).unwrap();

        match alphabet_config.mode {
            crate::core::config::EncodingMode::ByteRange => {
                let start = alphabet_config.start_codepoint.unwrap();
                Dictionary::new_with_mode_and_range(
                    Vec::new(),
                    alphabet_config.mode.clone(),
                    None,
                    Some(start),
                )
                .unwrap()
            }
            _ => {
                let chars: Vec<char> = alphabet_config.chars.chars().collect();
                let padding = alphabet_config
                    .padding
                    .as_ref()
                    .and_then(|s| s.chars().next());
                Dictionary::new_with_mode(chars, alphabet_config.mode.clone(), padding).unwrap()
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
