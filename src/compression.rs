use std::io::{Read, Write};

/// Supported compression algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    Gzip,
    Zstd,
    Brotli,
    Lz4,
    Snappy,
    Lzma,
}

impl CompressionAlgorithm {
    /// Parse compression algorithm from string.
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "gzip" | "gz" => Ok(CompressionAlgorithm::Gzip),
            "zstd" | "zst" => Ok(CompressionAlgorithm::Zstd),
            "brotli" | "br" => Ok(CompressionAlgorithm::Brotli),
            "lz4" => Ok(CompressionAlgorithm::Lz4),
            "snappy" | "snap" => Ok(CompressionAlgorithm::Snappy),
            "lzma" | "xz" => Ok(CompressionAlgorithm::Lzma),
            _ => Err(format!("Unknown compression algorithm: {}", s)),
        }
    }
    
    pub fn as_str(&self) -> &str {
        match self {
            CompressionAlgorithm::Gzip => "gzip",
            CompressionAlgorithm::Zstd => "zstd",
            CompressionAlgorithm::Brotli => "brotli",
            CompressionAlgorithm::Lz4 => "lz4",
            CompressionAlgorithm::Snappy => "snappy",
            CompressionAlgorithm::Lzma => "lzma",
        }
    }
}

/// Compress data using the specified algorithm and level.
pub fn compress(data: &[u8], algorithm: CompressionAlgorithm, level: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    match algorithm {
        CompressionAlgorithm::Gzip => compress_gzip(data, level),
        CompressionAlgorithm::Zstd => compress_zstd(data, level),
        CompressionAlgorithm::Brotli => compress_brotli(data, level),
        CompressionAlgorithm::Lz4 => compress_lz4(data, level),
        CompressionAlgorithm::Snappy => compress_snappy(data, level),
        CompressionAlgorithm::Lzma => compress_lzma(data, level),
    }
}

/// Decompress data using the specified algorithm.
pub fn decompress(data: &[u8], algorithm: CompressionAlgorithm) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    match algorithm {
        CompressionAlgorithm::Gzip => decompress_gzip(data),
        CompressionAlgorithm::Zstd => decompress_zstd(data),
        CompressionAlgorithm::Brotli => decompress_brotli(data),
        CompressionAlgorithm::Lz4 => decompress_lz4(data),
        CompressionAlgorithm::Snappy => decompress_snappy(data),
        CompressionAlgorithm::Lzma => decompress_lzma(data),
    }
}

fn compress_gzip(data: &[u8], level: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    
    let mut encoder = GzEncoder::new(Vec::new(), Compression::new(level));
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use flate2::read::GzDecoder;
    
    let mut decoder = GzDecoder::new(data);
    let mut result = Vec::new();
    decoder.read_to_end(&mut result)?;
    Ok(result)
}

fn compress_zstd(data: &[u8], level: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(zstd::encode_all(data, level as i32)?)
}

fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(zstd::decode_all(data)?)
}

fn compress_brotli(data: &[u8], level: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    let mut reader = brotli::CompressorReader::new(data, 4096, level, 22);
    reader.read_to_end(&mut result)?;
    Ok(result)
}

fn decompress_brotli(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    let mut reader = brotli::Decompressor::new(data, 4096);
    reader.read_to_end(&mut result)?;
    Ok(result)
}

fn compress_lz4(data: &[u8], _level: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // LZ4 doesn't use compression levels in the same way
    Ok(lz4::block::compress(data, None, false)?)
}

fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // We need to know the uncompressed size for LZ4, but we don't have it
    // Use a reasonable max size (100MB)
    Ok(lz4::block::decompress(data, Some(100 * 1024 * 1024))?)
}

fn compress_snappy(data: &[u8], _level: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Snappy doesn't support compression levels
    let mut encoder = snap::raw::Encoder::new();
    Ok(encoder.compress_vec(data)?)
}

fn decompress_snappy(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut decoder = snap::raw::Decoder::new();
    Ok(decoder.decompress_vec(data)?)
}

fn compress_lzma(data: &[u8], level: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use xz2::write::XzEncoder;
    
    let mut encoder = XzEncoder::new(Vec::new(), level);
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

fn decompress_lzma(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use xz2::read::XzDecoder;
    
    let mut decoder = XzDecoder::new(data);
    let mut result = Vec::new();
    decoder.read_to_end(&mut result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gzip_roundtrip() {
        let data = b"Hello, world! This is a test of gzip compression.";
        let compressed = compress(data, CompressionAlgorithm::Gzip, 6).unwrap();
        let decompressed = decompress(&compressed, CompressionAlgorithm::Gzip).unwrap();
        assert_eq!(data.as_ref(), decompressed.as_slice());
    }
    
    #[test]
    fn test_zstd_roundtrip() {
        let data = b"Hello, world! This is a test of zstd compression.";
        let compressed = compress(data, CompressionAlgorithm::Zstd, 3).unwrap();
        let decompressed = decompress(&compressed, CompressionAlgorithm::Zstd).unwrap();
        assert_eq!(data.as_ref(), decompressed.as_slice());
    }
    
    #[test]
    fn test_brotli_roundtrip() {
        let data = b"Hello, world! This is a test of brotli compression.";
        let compressed = compress(data, CompressionAlgorithm::Brotli, 6).unwrap();
        let decompressed = decompress(&compressed, CompressionAlgorithm::Brotli).unwrap();
        assert_eq!(data.as_ref(), decompressed.as_slice());
    }
    
    #[test]
    fn test_lz4_roundtrip() {
        let data = b"Hello, world! This is a test of lz4 compression.";
        let compressed = compress(data, CompressionAlgorithm::Lz4, 0).unwrap();
        let decompressed = decompress(&compressed, CompressionAlgorithm::Lz4).unwrap();
        assert_eq!(data.as_ref(), decompressed.as_slice());
    }
    
    #[test]
    fn test_snappy_roundtrip() {
        let data = b"Hello, world! This is a test of snappy compression.";
        let compressed = compress(data, CompressionAlgorithm::Snappy, 0).unwrap();
        let decompressed = decompress(&compressed, CompressionAlgorithm::Snappy).unwrap();
        assert_eq!(data.as_ref(), decompressed.as_slice());
    }
    
    #[test]
    fn test_lzma_roundtrip() {
        let data = b"Hello, world! This is a test of lzma compression.";
        let compressed = compress(data, CompressionAlgorithm::Lzma, 6).unwrap();
        let decompressed = decompress(&compressed, CompressionAlgorithm::Lzma).unwrap();
        assert_eq!(data.as_ref(), decompressed.as_slice());
    }
}
