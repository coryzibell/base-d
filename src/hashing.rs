use sha2::{Sha224, Sha256, Sha384, Sha512, Digest};
use sha3::{Sha3_224, Sha3_256, Sha3_384, Sha3_512, Keccak224, Keccak256, Keccak384, Keccak512};
use blake2::{Blake2b512, Blake2s256};
use blake3::Hasher as Blake3Hasher;
use md5::Md5;

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
        }
    }
}

/// Compute hash of data using the specified algorithm.
pub fn hash(data: &[u8], algorithm: HashAlgorithm) -> Vec<u8> {
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
    }
}
