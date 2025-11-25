use blake2::{Blake2b512, Blake2s256};
use blake3::Hasher as Blake3Hasher;
use md5::Md5;
use sha2::{Digest, Sha224, Sha256, Sha384, Sha512};
use sha3::{Keccak224, Keccak256, Keccak384, Keccak512, Sha3_224, Sha3_256, Sha3_384, Sha3_512};
use std::hash::Hasher;
use twox_hash::xxhash3_128::Hasher as Xxh3Hash128;
use twox_hash::xxhash3_64::Hasher as Xxh3Hash64;
use twox_hash::{XxHash32, XxHash64};

/// Configuration for xxHash algorithms.
#[derive(Debug, Clone, Default)]
pub struct XxHashConfig {
    /// Seed value (0-u64::MAX)
    pub seed: u64,
    /// Secret for XXH3 variants (must be >= 136 bytes)
    pub secret: Option<Vec<u8>>,
}

impl XxHashConfig {
    /// Create config with a custom seed.
    pub fn with_seed(seed: u64) -> Self {
        Self { seed, secret: None }
    }

    /// Create config with seed and secret for XXH3 variants.
    /// Secret must be at least 136 bytes.
    pub fn with_secret(seed: u64, secret: Vec<u8>) -> Result<Self, String> {
        if secret.len() < 136 {
            return Err(format!(
                "XXH3 secret must be >= 136 bytes, got {}",
                secret.len()
            ));
        }
        Ok(Self {
            seed,
            secret: Some(secret),
        })
    }
}

/// Supported hash algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Md5,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
    Sha3_224,
    Sha3_256,
    Sha3_384,
    Sha3_512,
    Keccak224,
    Keccak256,
    Keccak384,
    Keccak512,
    Blake2b,
    Blake2s,
    Blake3,
    // CRC variants
    Crc32,
    Crc32c,
    Crc16,
    Crc64,
    // xxHash variants
    XxHash32,
    XxHash64,
    XxHash3_64,
    XxHash3_128,
}

impl HashAlgorithm {
    /// Parse hash algorithm from string.
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "md5" => Ok(HashAlgorithm::Md5),
            "sha224" | "sha-224" => Ok(HashAlgorithm::Sha224),
            "sha256" | "sha-256" => Ok(HashAlgorithm::Sha256),
            "sha384" | "sha-384" => Ok(HashAlgorithm::Sha384),
            "sha512" | "sha-512" => Ok(HashAlgorithm::Sha512),
            "sha3-224" | "sha3_224" => Ok(HashAlgorithm::Sha3_224),
            "sha3-256" | "sha3_256" => Ok(HashAlgorithm::Sha3_256),
            "sha3-384" | "sha3_384" => Ok(HashAlgorithm::Sha3_384),
            "sha3-512" | "sha3_512" => Ok(HashAlgorithm::Sha3_512),
            "keccak224" | "keccak-224" => Ok(HashAlgorithm::Keccak224),
            "keccak256" | "keccak-256" => Ok(HashAlgorithm::Keccak256),
            "keccak384" | "keccak-384" => Ok(HashAlgorithm::Keccak384),
            "keccak512" | "keccak-512" => Ok(HashAlgorithm::Keccak512),
            "blake2b" | "blake2b-512" => Ok(HashAlgorithm::Blake2b),
            "blake2s" | "blake2s-256" => Ok(HashAlgorithm::Blake2s),
            "blake3" => Ok(HashAlgorithm::Blake3),
            "crc32" => Ok(HashAlgorithm::Crc32),
            "crc32c" => Ok(HashAlgorithm::Crc32c),
            "crc16" => Ok(HashAlgorithm::Crc16),
            "crc64" => Ok(HashAlgorithm::Crc64),
            "xxhash32" | "xxh32" => Ok(HashAlgorithm::XxHash32),
            "xxhash64" | "xxh64" => Ok(HashAlgorithm::XxHash64),
            "xxhash3" | "xxh3" | "xxhash3-64" | "xxh3-64" => Ok(HashAlgorithm::XxHash3_64),
            "xxhash3-128" | "xxh3-128" => Ok(HashAlgorithm::XxHash3_128),
            _ => Err(format!("Unknown hash algorithm: {}", s)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            HashAlgorithm::Md5 => "md5",
            HashAlgorithm::Sha224 => "sha224",
            HashAlgorithm::Sha256 => "sha256",
            HashAlgorithm::Sha384 => "sha384",
            HashAlgorithm::Sha512 => "sha512",
            HashAlgorithm::Sha3_224 => "sha3-224",
            HashAlgorithm::Sha3_256 => "sha3-256",
            HashAlgorithm::Sha3_384 => "sha3-384",
            HashAlgorithm::Sha3_512 => "sha3-512",
            HashAlgorithm::Keccak224 => "keccak224",
            HashAlgorithm::Keccak256 => "keccak256",
            HashAlgorithm::Keccak384 => "keccak384",
            HashAlgorithm::Keccak512 => "keccak512",
            HashAlgorithm::Blake2b => "blake2b",
            HashAlgorithm::Blake2s => "blake2s",
            HashAlgorithm::Blake3 => "blake3",
            HashAlgorithm::Crc32 => "crc32",
            HashAlgorithm::Crc32c => "crc32c",
            HashAlgorithm::Crc16 => "crc16",
            HashAlgorithm::Crc64 => "crc64",
            HashAlgorithm::XxHash32 => "xxhash32",
            HashAlgorithm::XxHash64 => "xxhash64",
            HashAlgorithm::XxHash3_64 => "xxhash3-64",
            HashAlgorithm::XxHash3_128 => "xxhash3-128",
        }
    }

    /// Get the output size in bytes for this algorithm.
    pub fn output_size(&self) -> usize {
        match self {
            HashAlgorithm::Md5 => 16,
            HashAlgorithm::Sha224 => 28,
            HashAlgorithm::Sha256 => 32,
            HashAlgorithm::Sha384 => 48,
            HashAlgorithm::Sha512 => 64,
            HashAlgorithm::Sha3_224 => 28,
            HashAlgorithm::Sha3_256 => 32,
            HashAlgorithm::Sha3_384 => 48,
            HashAlgorithm::Sha3_512 => 64,
            HashAlgorithm::Keccak224 => 28,
            HashAlgorithm::Keccak256 => 32,
            HashAlgorithm::Keccak384 => 48,
            HashAlgorithm::Keccak512 => 64,
            HashAlgorithm::Blake2b => 64,
            HashAlgorithm::Blake2s => 32,
            HashAlgorithm::Blake3 => 32,
            HashAlgorithm::Crc16 => 2,
            HashAlgorithm::Crc32 => 4,
            HashAlgorithm::Crc32c => 4,
            HashAlgorithm::Crc64 => 8,
            HashAlgorithm::XxHash32 => 4,
            HashAlgorithm::XxHash64 => 8,
            HashAlgorithm::XxHash3_64 => 8,
            HashAlgorithm::XxHash3_128 => 16,
        }
    }
}

/// Compute hash of data using the specified algorithm.
/// Uses default configuration (seed = 0, no secret).
pub fn hash(data: &[u8], algorithm: HashAlgorithm) -> Vec<u8> {
    hash_with_config(data, algorithm, &XxHashConfig::default())
}

/// Compute hash of data using the specified algorithm with custom configuration.
pub fn hash_with_config(data: &[u8], algorithm: HashAlgorithm, config: &XxHashConfig) -> Vec<u8> {
    match algorithm {
        HashAlgorithm::Md5 => {
            let mut hasher = Md5::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Sha224 => {
            let mut hasher = Sha224::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Sha384 => {
            let mut hasher = Sha384::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Sha512 => {
            let mut hasher = Sha512::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Sha3_224 => {
            let mut hasher = Sha3_224::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Sha3_256 => {
            let mut hasher = Sha3_256::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Sha3_384 => {
            let mut hasher = Sha3_384::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Sha3_512 => {
            let mut hasher = Sha3_512::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Keccak224 => {
            let mut hasher = Keccak224::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Keccak256 => {
            let mut hasher = Keccak256::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Keccak384 => {
            let mut hasher = Keccak384::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Keccak512 => {
            let mut hasher = Keccak512::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Blake2b => {
            let mut hasher = Blake2b512::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Blake2s => {
            let mut hasher = Blake2s256::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Blake3 => {
            let mut hasher = Blake3Hasher::new();
            hasher.update(data);
            hasher.finalize().as_bytes().to_vec()
        }
        HashAlgorithm::Crc16 => {
            let crc = crc::Crc::<u16>::new(&crc::CRC_16_IBM_SDLC);
            let result = crc.checksum(data);
            result.to_be_bytes().to_vec()
        }
        HashAlgorithm::Crc32 => {
            let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
            let result = crc.checksum(data);
            result.to_be_bytes().to_vec()
        }
        HashAlgorithm::Crc32c => {
            let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);
            let result = crc.checksum(data);
            result.to_be_bytes().to_vec()
        }
        HashAlgorithm::Crc64 => {
            let crc = crc::Crc::<u64>::new(&crc::CRC_64_ECMA_182);
            let result = crc.checksum(data);
            result.to_be_bytes().to_vec()
        }
        HashAlgorithm::XxHash32 => {
            let mut hasher = XxHash32::with_seed(config.seed as u32);
            hasher.write(data);
            (hasher.finish() as u32).to_be_bytes().to_vec()
        }
        HashAlgorithm::XxHash64 => {
            let mut hasher = XxHash64::with_seed(config.seed);
            hasher.write(data);
            hasher.finish().to_be_bytes().to_vec()
        }
        HashAlgorithm::XxHash3_64 => {
            let mut hasher = if let Some(ref secret) = config.secret {
                Xxh3Hash64::with_seed_and_secret(config.seed, secret.as_slice()).expect(
                    "XXH3 secret validation should have been done in XxHashConfig::with_secret",
                )
            } else {
                Xxh3Hash64::with_seed(config.seed)
            };
            hasher.write(data);
            hasher.finish().to_be_bytes().to_vec()
        }
        HashAlgorithm::XxHash3_128 => {
            let mut hasher = if let Some(ref secret) = config.secret {
                Xxh3Hash128::with_seed_and_secret(config.seed, secret.as_slice()).expect(
                    "XXH3 secret validation should have been done in XxHashConfig::with_secret",
                )
            } else {
                Xxh3Hash128::with_seed(config.seed)
            };
            hasher.write(data);
            hasher.finish_128().to_be_bytes().to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md5() {
        let data = b"hello world";
        let hash = hash(data, HashAlgorithm::Md5);
        assert_eq!(hash.len(), 16);
        // MD5 of "hello world" is 5eb63bbbe01eeed093cb22bb8f5acdc3
        assert_eq!(hex::encode(&hash), "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn test_sha256() {
        let data = b"hello world";
        let hash = hash(data, HashAlgorithm::Sha256);
        assert_eq!(hash.len(), 32);
        // SHA-256 of "hello world"
        assert_eq!(
            hex::encode(&hash),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_sha512() {
        let data = b"hello world";
        let hash = hash(data, HashAlgorithm::Sha512);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_sha3_256() {
        let data = b"hello world";
        let hash = hash(data, HashAlgorithm::Sha3_256);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_blake2b() {
        let data = b"hello world";
        let hash = hash(data, HashAlgorithm::Blake2b);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_blake2s() {
        let data = b"hello world";
        let hash = hash(data, HashAlgorithm::Blake2s);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_blake3() {
        let data = b"hello world";
        let hash = hash(data, HashAlgorithm::Blake3);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_empty_input() {
        let data = b"";
        let hash = hash(data, HashAlgorithm::Sha256);
        assert_eq!(hash.len(), 32);
        // SHA-256 of empty string
        assert_eq!(
            hex::encode(&hash),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_output_sizes() {
        assert_eq!(HashAlgorithm::Md5.output_size(), 16);
        assert_eq!(HashAlgorithm::Sha256.output_size(), 32);
        assert_eq!(HashAlgorithm::Sha512.output_size(), 64);
        assert_eq!(HashAlgorithm::Blake3.output_size(), 32);
        assert_eq!(HashAlgorithm::Crc16.output_size(), 2);
        assert_eq!(HashAlgorithm::Crc32.output_size(), 4);
        assert_eq!(HashAlgorithm::Crc64.output_size(), 8);
        assert_eq!(HashAlgorithm::XxHash32.output_size(), 4);
        assert_eq!(HashAlgorithm::XxHash64.output_size(), 8);
        assert_eq!(HashAlgorithm::XxHash3_64.output_size(), 8);
        assert_eq!(HashAlgorithm::XxHash3_128.output_size(), 16);
    }

    #[test]
    fn test_crc32() {
        let data = b"hello world";
        let result = hash(data, HashAlgorithm::Crc32);
        assert_eq!(result.len(), 4);
        // CRC32 is deterministic
        let result2 = hash(data, HashAlgorithm::Crc32);
        assert_eq!(result, result2);
    }

    #[test]
    fn test_crc32c() {
        let data = b"hello world";
        let result = hash(data, HashAlgorithm::Crc32c);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_crc16() {
        let data = b"hello world";
        let result = hash(data, HashAlgorithm::Crc16);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_crc64() {
        let data = b"hello world";
        let result = hash(data, HashAlgorithm::Crc64);
        assert_eq!(result.len(), 8);
    }

    #[test]
    fn test_xxhash32() {
        let data = b"hello world";
        let result = hash(data, HashAlgorithm::XxHash32);
        assert_eq!(result.len(), 4);
        // xxHash is deterministic with same seed
        let result2 = hash(data, HashAlgorithm::XxHash32);
        assert_eq!(result, result2);
    }

    #[test]
    fn test_xxhash64() {
        let data = b"hello world";
        let result = hash(data, HashAlgorithm::XxHash64);
        assert_eq!(result.len(), 8);
    }

    #[test]
    fn test_xxhash3_64() {
        let data = b"hello world";
        let result = hash(data, HashAlgorithm::XxHash3_64);
        assert_eq!(result.len(), 8);
    }

    #[test]
    fn test_xxhash3_128() {
        let data = b"hello world";
        let result = hash(data, HashAlgorithm::XxHash3_128);
        assert_eq!(result.len(), 16);
    }

    #[test]
    fn test_xxhash_config_default() {
        let config = XxHashConfig::default();
        assert_eq!(config.seed, 0);
        assert!(config.secret.is_none());
    }

    #[test]
    fn test_xxhash_config_secret_too_short() {
        let result = XxHashConfig::with_secret(0, vec![0u8; 100]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("136 bytes"));
    }

    #[test]
    fn test_xxhash_config_secret_valid() {
        let result = XxHashConfig::with_secret(42, vec![0u8; 136]);
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.seed, 42);
        assert_eq!(config.secret.as_ref().unwrap().len(), 136);
    }

    #[test]
    fn test_hash_seed_changes_output() {
        let data = b"test";
        let h1 = hash_with_config(data, HashAlgorithm::XxHash64, &XxHashConfig::with_seed(0));
        let h2 = hash_with_config(data, HashAlgorithm::XxHash64, &XxHashConfig::with_seed(42));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_backward_compatibility() {
        let data = b"test";
        let old = hash(data, HashAlgorithm::XxHash64);
        let new = hash_with_config(data, HashAlgorithm::XxHash64, &XxHashConfig::default());
        assert_eq!(old, new);
    }

    #[test]
    fn test_xxhash3_with_seed() {
        let data = b"test data for secret hashing";

        // Test that different seeds produce different hashes
        let h1 = hash_with_config(data, HashAlgorithm::XxHash3_64, &XxHashConfig::with_seed(0));
        let h2 = hash_with_config(
            data,
            HashAlgorithm::XxHash3_64,
            &XxHashConfig::with_seed(123),
        );
        assert_ne!(h1, h2, "Different seeds should produce different hashes");

        // Test that same seed produces same hash
        let h3 = hash_with_config(
            data,
            HashAlgorithm::XxHash3_64,
            &XxHashConfig::with_seed(123),
        );
        assert_eq!(h2, h3, "Same seed should produce same hash");
    }

    #[test]
    fn test_xxhash32_with_seed() {
        let data = b"test";
        let h1 = hash_with_config(data, HashAlgorithm::XxHash32, &XxHashConfig::with_seed(0));
        let h2 = hash_with_config(data, HashAlgorithm::XxHash32, &XxHashConfig::with_seed(999));
        assert_ne!(h1, h2);
    }
}
