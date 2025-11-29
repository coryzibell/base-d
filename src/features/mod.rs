//! Optional features module.
//!
//! This module contains optional functionality for compression, hashing,
//! and dictionary detection. These features are organized separately from
//! the core encoding/decoding functionality.

pub mod compression;
pub mod detection;
pub mod hashing;

// Re-export main types and functions for convenience
pub use compression::{CompressionAlgorithm, compress, decompress};
pub use detection::{DictionaryDetector, DictionaryMatch, detect_dictionary};
pub use hashing::{HashAlgorithm, XxHashConfig, hash, hash_with_config};
