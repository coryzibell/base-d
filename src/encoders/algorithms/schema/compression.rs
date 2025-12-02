use super::types::SchemaError;
use crate::features::compression::{CompressionAlgorithm, compress, decompress};

/// Algorithm byte prefix values
const ALGO_NONE: u8 = 0x00;
const ALGO_BROTLI: u8 = 0x01;
const ALGO_LZ4: u8 = 0x02;
const ALGO_ZSTD: u8 = 0x03;

/// Default compression level for schema encoding
const DEFAULT_LEVEL: u32 = 6;

/// Compression algorithms for schema encoding
///
/// These algorithms are applied to the binary payload before display96 encoding.
/// The algorithm is stored as a 1-byte prefix in the compressed payload.
///
/// # Algorithms
///
/// * `Brotli` - Best compression ratio (prefix: 0x01)
/// * `Lz4` - Fastest compression/decompression (prefix: 0x02)
/// * `Zstd` - Balanced compression and speed (prefix: 0x03)
///
/// All algorithms use default compression level 6.
///
/// # Examples
///
/// ```ignore
/// use base_d::{encode_schema, SchemaCompressionAlgo};
///
/// let json = r#"{"data":[1,2,3,4,5]}"#;
///
/// // Best compression
/// let encoded = encode_schema(json, Some(SchemaCompressionAlgo::Brotli))?;
///
/// // Fastest
/// let encoded = encode_schema(json, Some(SchemaCompressionAlgo::Lz4))?;
///
/// // Balanced
/// let encoded = encode_schema(json, Some(SchemaCompressionAlgo::Zstd))?;
/// ```
#[derive(Clone, Copy, Debug)]
pub enum SchemaCompressionAlgo {
    Brotli,
    Lz4,
    Zstd,
}

/// Apply compression to binary data with algorithm prefix
///
/// Compresses the binary payload and prepends a 1-byte algorithm identifier.
/// This allows automatic detection of the compression algorithm during decoding.
///
/// # Format
///
/// ```text
/// [algo_byte: u8][compressed_payload: bytes]
/// ```
///
/// # Algorithm Bytes
///
/// * `0x00` - No compression (payload is raw binary)
/// * `0x01` - Brotli (level 6)
/// * `0x02` - LZ4 (level 6)
/// * `0x03` - Zstd (level 6)
///
/// # Arguments
///
/// * `binary` - Raw binary data to compress
/// * `algo` - Optional compression algorithm (None = no compression)
///
/// # Returns
///
/// Returns a byte vector with algorithm prefix followed by (possibly compressed) payload.
///
/// # Errors
///
/// * `SchemaError::Compression` - Compression failure
///
/// # Examples
///
/// ```ignore
/// use base_d::{compress_with_prefix, SchemaCompressionAlgo};
///
/// let data = b"Hello, world!";
///
/// // No compression (returns [0x00, ...data])
/// let result = compress_with_prefix(data, None)?;
///
/// // With brotli (returns [0x01, ...compressed])
/// let result = compress_with_prefix(data, Some(SchemaCompressionAlgo::Brotli))?;
/// ```
pub fn compress_with_prefix(
    binary: &[u8],
    algo: Option<SchemaCompressionAlgo>,
) -> Result<Vec<u8>, SchemaError> {
    let (algo_byte, compressed) = match algo {
        None => (ALGO_NONE, binary.to_vec()),
        Some(SchemaCompressionAlgo::Brotli) => {
            let c = compress(binary, CompressionAlgorithm::Brotli, DEFAULT_LEVEL)
                .map_err(|e| SchemaError::Compression(e.to_string()))?;
            (ALGO_BROTLI, c)
        }
        Some(SchemaCompressionAlgo::Lz4) => {
            let c = compress(binary, CompressionAlgorithm::Lz4, DEFAULT_LEVEL)
                .map_err(|e| SchemaError::Compression(e.to_string()))?;
            (ALGO_LZ4, c)
        }
        Some(SchemaCompressionAlgo::Zstd) => {
            let c = compress(binary, CompressionAlgorithm::Zstd, DEFAULT_LEVEL)
                .map_err(|e| SchemaError::Compression(e.to_string()))?;
            (ALGO_ZSTD, c)
        }
    };

    // Prepend algorithm byte
    let mut result = Vec::with_capacity(1 + compressed.len());
    result.push(algo_byte);
    result.extend_from_slice(&compressed);
    Ok(result)
}

/// Decompress binary data using algorithm prefix
///
/// Reads the first byte to determine the compression algorithm, then decompresses
/// the remaining payload. Supports all algorithms from [`compress_with_prefix`].
///
/// # Arguments
///
/// * `binary` - Byte slice with algorithm prefix + payload
///
/// # Returns
///
/// Returns the decompressed binary data (or raw data if uncompressed).
///
/// # Errors
///
/// * `SchemaError::UnexpectedEndOfData` - Empty input (missing prefix byte)
/// * `SchemaError::InvalidCompressionAlgorithm` - Invalid algorithm byte
/// * `SchemaError::Decompression` - Decompression failure
///
/// # Examples
///
/// ```ignore
/// use base_d::decompress_with_prefix;
///
/// // Decompress data (auto-detects algorithm from prefix)
/// let compressed = vec![0x01, /* brotli compressed bytes */];
/// let data = decompress_with_prefix(&compressed)?;
///
/// // Uncompressed data (prefix 0x00)
/// let uncompressed = vec![0x00, 0x48, 0x65, 0x6C, 0x6C, 0x6F];
/// let data = decompress_with_prefix(&uncompressed)?;
/// // Returns: b"Hello"
/// ```
pub fn decompress_with_prefix(binary: &[u8]) -> Result<Vec<u8>, SchemaError> {
    if binary.is_empty() {
        return Err(SchemaError::UnexpectedEndOfData {
            context: "compression prefix".to_string(),
            position: 0,
        });
    }

    let algo_byte = binary[0];
    let payload = &binary[1..];

    match algo_byte {
        ALGO_NONE => Ok(payload.to_vec()),
        ALGO_BROTLI => decompress(payload, CompressionAlgorithm::Brotli)
            .map_err(|e| SchemaError::Decompression(e.to_string())),
        ALGO_LZ4 => decompress(payload, CompressionAlgorithm::Lz4)
            .map_err(|e| SchemaError::Decompression(e.to_string())),
        ALGO_ZSTD => decompress(payload, CompressionAlgorithm::Zstd)
            .map_err(|e| SchemaError::Decompression(e.to_string())),
        _ => Err(SchemaError::InvalidCompressionAlgorithm(algo_byte)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_compression() {
        let data = b"Hello, world!";
        let compressed = compress_with_prefix(data, None).unwrap();

        // Should have algo byte + raw data
        assert_eq!(compressed[0], ALGO_NONE);
        assert_eq!(&compressed[1..], data);

        let decompressed = decompress_with_prefix(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_brotli_roundtrip() {
        let data = b"Hello, world! This is a test of brotli compression in schema encoding.";
        let compressed = compress_with_prefix(data, Some(SchemaCompressionAlgo::Brotli)).unwrap();

        assert_eq!(compressed[0], ALGO_BROTLI);

        let decompressed = decompress_with_prefix(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_lz4_roundtrip() {
        let data = b"Hello, world! This is a test of lz4 compression in schema encoding.";
        let compressed = compress_with_prefix(data, Some(SchemaCompressionAlgo::Lz4)).unwrap();

        assert_eq!(compressed[0], ALGO_LZ4);

        let decompressed = decompress_with_prefix(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_zstd_roundtrip() {
        let data = b"Hello, world! This is a test of zstd compression in schema encoding.";
        let compressed = compress_with_prefix(data, Some(SchemaCompressionAlgo::Zstd)).unwrap();

        assert_eq!(compressed[0], ALGO_ZSTD);

        let decompressed = decompress_with_prefix(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_small_payload() {
        // Small payloads may expand with compression overhead
        let data = b"Hi";
        let compressed = compress_with_prefix(data, Some(SchemaCompressionAlgo::Brotli)).unwrap();
        let decompressed = decompress_with_prefix(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_empty_payload() {
        let data = b"";
        let compressed = compress_with_prefix(data, None).unwrap();
        assert_eq!(compressed, vec![ALGO_NONE]);

        let decompressed = decompress_with_prefix(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_invalid_algorithm() {
        let invalid = vec![0xFF, 0x01, 0x02, 0x03];
        let result = decompress_with_prefix(&invalid);
        assert!(matches!(
            result,
            Err(SchemaError::InvalidCompressionAlgorithm(0xFF))
        ));
    }

    #[test]
    fn test_missing_prefix() {
        let result = decompress_with_prefix(&[]);
        assert!(matches!(
            result,
            Err(SchemaError::UnexpectedEndOfData { .. })
        ));
    }
}
