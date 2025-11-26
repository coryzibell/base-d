//! LUT-based SIMD codecs for arbitrary alphabets
//!
//! This module provides SIMD acceleration for non-sequential alphabets
//! through lookup table techniques.

pub mod small;

pub use small::SmallLutCodec;
