//! Specialized SIMD implementations for different bit-widths
//!
//! Each module contains the NEON-optimized encode/decode logic for a specific
//! bit-width encoding (4-bit, 5-bit, 6-bit, 8-bit).

pub mod base16;
pub mod base256;
pub mod base32;
pub mod base64;
