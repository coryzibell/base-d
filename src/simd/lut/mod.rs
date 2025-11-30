//! LUT-based SIMD codecs for arbitrary dictionaries
//!
//! This module provides SIMD acceleration for non-sequential dictionaries
//! through lookup table techniques.

pub mod base16;
mod base32; // Base32-specific implementations for Base64LutCodec
pub mod base64;
mod common;
pub mod gapped;

pub use base16::SmallLutCodec;
pub use base64::Base64LutCodec;
pub use gapped::GappedSequentialCodec;
