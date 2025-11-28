//! LUT-based SIMD codecs for arbitrary dictionaries
//!
//! This module provides SIMD acceleration for non-sequential dictionaries
//! through lookup table techniques.

pub mod large;
pub mod small;

pub use large::LargeLutCodec;
pub use small::SmallLutCodec;
