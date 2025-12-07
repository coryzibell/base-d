pub mod json;
#[allow(dead_code)]
pub mod markdown;
pub mod markdown_doc;

use crate::encoders::algorithms::schema::types::IntermediateRepresentation;

/// Trait for parsing input formats into intermediate representation
pub trait InputParser {
    type Error;

    /// Parse input string into intermediate representation
    fn parse(input: &str) -> Result<IntermediateRepresentation, Self::Error>;
}

pub use json::JsonParser;
pub use markdown_doc::MarkdownDocParser;
