//! Convenience functions for common encoding patterns.
//!
//! These functions combine hashing/compression with encoding in a single call,
//! using random dictionary selection for varied output.

use crate::{CompressionAlgorithm, DictionaryRegistry, HashAlgorithm, compress, encode, hash};

/// Result of a hash + encode operation.
#[derive(Debug, Clone)]
pub struct HashEncodeResult {
    /// The encoded output
    pub encoded: String,
    /// The hash algorithm used
    pub hash_algo: HashAlgorithm,
    /// Name of the dictionary used for encoding
    pub dictionary_name: String,
}

/// Result of a compress + encode operation.
#[derive(Debug, Clone)]
pub struct CompressEncodeResult {
    /// The encoded output
    pub encoded: String,
    /// The compression algorithm used
    pub compress_algo: CompressionAlgorithm,
    /// Name of the dictionary used for encoding
    pub dictionary_name: String,
}

/// Hash data with a random algorithm and encode with a random dictionary.
///
/// # Example
/// ```
/// use base_d::{DictionaryRegistry, convenience::hash_encode};
///
/// let registry = DictionaryRegistry::load_default().unwrap();
/// let result = hash_encode(b"Hello, world!", &registry).unwrap();
/// println!("Encoded: {}", result.encoded);
/// println!("Hash: {}", result.hash_algo.as_str());
/// println!("Dictionary: {}", result.dictionary_name);
/// ```
pub fn hash_encode(
    data: &[u8],
    registry: &DictionaryRegistry,
) -> Result<HashEncodeResult, Box<dyn std::error::Error>> {
    let algo = HashAlgorithm::random();
    hash_encode_with(data, algo, registry)
}

/// Hash data with a specific algorithm and encode with a random dictionary.
pub fn hash_encode_with(
    data: &[u8],
    algo: HashAlgorithm,
    registry: &DictionaryRegistry,
) -> Result<HashEncodeResult, Box<dyn std::error::Error>> {
    let hashed = hash(data, algo);
    let (dict_name, dict) = registry.random()?;
    let encoded = encode(&hashed, &dict);

    Ok(HashEncodeResult {
        encoded,
        hash_algo: algo,
        dictionary_name: dict_name,
    })
}

/// Compress data with a random algorithm and encode with a random dictionary.
///
/// # Example
/// ```
/// use base_d::{DictionaryRegistry, convenience::compress_encode};
///
/// let registry = DictionaryRegistry::load_default().unwrap();
/// let result = compress_encode(b"Hello, world!", &registry).unwrap();
/// println!("Encoded: {}", result.encoded);
/// println!("Compression: {}", result.compress_algo.as_str());
/// println!("Dictionary: {}", result.dictionary_name);
/// ```
pub fn compress_encode(
    data: &[u8],
    registry: &DictionaryRegistry,
) -> Result<CompressEncodeResult, Box<dyn std::error::Error>> {
    let algo = CompressionAlgorithm::random();
    compress_encode_with(data, algo, registry)
}

/// Compress data with a specific algorithm and encode with a random dictionary.
pub fn compress_encode_with(
    data: &[u8],
    algo: CompressionAlgorithm,
    registry: &DictionaryRegistry,
) -> Result<CompressEncodeResult, Box<dyn std::error::Error>> {
    let level = algo.default_level();
    let compressed = compress(data, algo, level)?;
    let (dict_name, dict) = registry.random()?;
    let encoded = encode(&compressed, &dict);

    Ok(CompressEncodeResult {
        encoded,
        compress_algo: algo,
        dictionary_name: dict_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_encode() {
        let registry = DictionaryRegistry::load_default().unwrap();
        let result = hash_encode(b"test data", &registry).unwrap();
        assert!(!result.encoded.is_empty());
        assert!(!result.dictionary_name.is_empty());
    }

    #[test]
    fn test_compress_encode() {
        let registry = DictionaryRegistry::load_default().unwrap();
        let result = compress_encode(b"test data for compression", &registry).unwrap();
        assert!(!result.encoded.is_empty());
        assert!(!result.dictionary_name.is_empty());
    }

    #[test]
    fn test_hash_encode_with_specific_algo() {
        let registry = DictionaryRegistry::load_default().unwrap();
        let result = hash_encode_with(b"test", HashAlgorithm::Sha256, &registry).unwrap();
        assert_eq!(result.hash_algo, HashAlgorithm::Sha256);
    }

    #[test]
    fn test_compress_encode_with_specific_algo() {
        let registry = DictionaryRegistry::load_default().unwrap();
        let result =
            compress_encode_with(b"test data", CompressionAlgorithm::Gzip, &registry).unwrap();
        assert_eq!(result.compress_algo, CompressionAlgorithm::Gzip);
    }

    #[test]
    fn test_random_hash() {
        // Just verify it doesn't panic and returns valid algorithms
        for _ in 0..10 {
            let algo = HashAlgorithm::random();
            assert!(HashAlgorithm::all().contains(&algo));
        }
    }

    #[test]
    fn test_random_compress() {
        // Just verify it doesn't panic and returns valid algorithms
        for _ in 0..10 {
            let algo = CompressionAlgorithm::random();
            assert!(CompressionAlgorithm::all().contains(&algo));
        }
    }
}
