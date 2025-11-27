//! Optional features module.
//!
//! This module contains optional functionality for compression, hashing,
//! and dictionary detection. These features are organized separately from
//! the core encoding/decoding functionality.

pub mod compression;
pub mod detection;
pub mod hashing;

// Re-export main types and functions for convenience
pub use compression::{compress, decompress, CompressionAlgorithm};
pub use detection::{detect_dictionary, DictionaryDetector, DictionaryMatch};
pub use hashing::{hash, hash_with_config, HashAlgorithm, XxHashConfig};
