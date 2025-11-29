pub mod byte_range;
pub mod chunked;
pub mod errors;
pub mod math;

// Re-export error types for public API
pub use errors::{DecodeError, DictionaryNotFoundError, find_closest_dictionary};
