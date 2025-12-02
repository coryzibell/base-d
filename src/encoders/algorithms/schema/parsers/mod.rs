pub mod json;

use crate::encoders::algorithms::schema::types::IntermediateRepresentation;

/// Trait for parsing input formats into intermediate representation
pub trait InputParser {
    type Error;

    /// Parse input string into intermediate representation
    fn parse(input: &str) -> Result<IntermediateRepresentation, Self::Error>;
}

pub use json::JsonParser;
