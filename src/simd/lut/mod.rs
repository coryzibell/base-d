//! LUT-based SIMD codecs for arbitrary alphabets
//!
//! This module provides SIMD acceleration for non-sequential alphabets
//! through lookup table techniques.

pub mod large;
pub mod small;

pub use large::LargeLutCodec;
pub use small::SmallLutCodec;
