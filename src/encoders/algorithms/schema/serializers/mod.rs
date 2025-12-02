pub mod json;

use crate::encoders::algorithms::schema::types::IntermediateRepresentation;

/// Trait for serializing intermediate representation to output formats
pub trait OutputSerializer {
    type Error;

    /// Serialize intermediate representation to output string
    fn serialize(ir: &IntermediateRepresentation, pretty: bool) -> Result<String, Self::Error>;
}

pub use json::JsonSerializer;
