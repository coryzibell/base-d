//! SIMD-accelerated encoding/decoding implementations
//!
//! This module provides platform-specific SIMD optimizations for encoding
//! and decoding operations. Runtime CPU feature detection is used to
//! automatically select the best implementation.

use std::sync::OnceLock;

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::{encode_base64_simd, decode_base64_simd};

// CPU feature detection cache
static HAS_AVX2: OnceLock<bool> = OnceLock::new();

#[cfg(target_arch = "x86_64")]
static HAS_SSSE3: OnceLock<bool> = OnceLock::new();

/// Check if AVX2 is available (cached after first call)
#[cfg(target_arch = "x86_64")]
pub fn has_avx2() -> bool {
    *HAS_AVX2.get_or_init(|| is_x86_feature_detected!("avx2"))
}

/// Check if SSSE3 is available (cached after first call)
#[cfg(target_arch = "x86_64")]
pub fn has_ssse3() -> bool {
    *HAS_SSSE3.get_or_init(|| is_x86_feature_detected!("ssse3"))
}

#[cfg(not(target_arch = "x86_64"))]
pub fn has_avx2() -> bool {
    false
}

#[cfg(not(target_arch = "x86_64"))]
pub fn has_ssse3() -> bool {
    false
}
