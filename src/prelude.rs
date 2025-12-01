//! Convenient re-exports for common usage.
//!
//! This module provides a single import for the most commonly used types
//! and functions in base-d.
//!
//! # Example
//!
//! ```
//! use base_d::prelude::*;
//!
//! let registry = DictionaryRegistry::load_default().unwrap();
//! let result = hash_encode(b"Hello", &registry).unwrap();
//! println!("{}", result.encoded);
//! ```

pub use crate::{
    CompressionAlgorithm,

    DecodeError,
    Dictionary,
    // Detection
    DictionaryDetector,
    DictionaryMatch,

    DictionaryRegistry,

    // Config
    EncodingMode,
    // Feature types
    HashAlgorithm,
    compress,
    // Convenience functions
    convenience::{
        CompressEncodeResult, HashEncodeResult, compress_encode, compress_encode_with, hash_encode,
        hash_encode_with,
    },

    decode,
    decompress,
    detect_dictionary,

    // Core encoding/decoding
    encode,
    // Lower-level functions if needed
    hash,
};
