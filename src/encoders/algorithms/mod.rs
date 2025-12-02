pub mod byte_range;
pub mod chunked;
pub mod errors;
pub mod radix;
pub mod schema;

// Re-export error types for public API
pub use errors::{DecodeError, DictionaryNotFoundError, find_closest_dictionary};

// Re-export schema functions for CLI
#[allow(unused_imports)]
pub use schema::{decode_schema, encode_schema};
