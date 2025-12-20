//! WebAssembly bindings for base-d
//!
//! Provides JavaScript-friendly API for encoding and decoding.

use wasm_bindgen::prelude::*;

use crate::core::config::DictionaryRegistry;
use crate::core::dictionary::Dictionary;
use crate::{decode as decode_internal, encode as encode_internal};

/// Error type for WASM operations
#[wasm_bindgen]
pub struct WasmError {
    message: String,
}

#[wasm_bindgen]
impl WasmError {
    /// Get the error message
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }
}

impl From<String> for WasmError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for WasmError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl From<crate::encoders::algorithms::DecodeError> for WasmError {
    fn from(err: crate::encoders::algorithms::DecodeError) -> Self {
        Self {
            message: format!("{:?}", err),
        }
    }
}

/// Encode bytes to a base64 string
///
/// # Arguments
///
/// * `data` - The binary data to encode
///
/// # Returns
///
/// Base64-encoded string
#[wasm_bindgen]
pub fn encode_base64(data: &[u8]) -> Result<String, WasmError> {
    let registry = DictionaryRegistry::load_default()
        .map_err(|e| WasmError::from(format!("Failed to load registry: {:?}", e)))?;

    let config = registry
        .get_dictionary("base64")
        .ok_or_else(|| WasmError::from("base64 dictionary not found"))?;

    let chars: Vec<char> = config.chars.chars().collect();
    let padding = config.padding.as_ref().and_then(|s| s.chars().next());
    let mut builder = Dictionary::builder()
        .chars(chars)
        .mode(config.effective_mode());
    if let Some(p) = padding {
        builder = builder.padding(p);
    }
    let dictionary = builder
        .build()
        .map_err(|e| WasmError::from(format!("Failed to build dictionary: {:?}", e)))?;

    Ok(encode_internal(data, &dictionary))
}

/// Decode a base64 string back to bytes
///
/// # Arguments
///
/// * `encoded` - The base64-encoded string
///
/// # Returns
///
/// Decoded binary data
#[wasm_bindgen]
pub fn decode_base64(encoded: &str) -> Result<Vec<u8>, WasmError> {
    let registry = DictionaryRegistry::load_default()
        .map_err(|e| WasmError::from(format!("Failed to load registry: {:?}", e)))?;

    let config = registry
        .get_dictionary("base64")
        .ok_or_else(|| WasmError::from("base64 dictionary not found"))?;

    let chars: Vec<char> = config.chars.chars().collect();
    let padding = config.padding.as_ref().and_then(|s| s.chars().next());
    let mut builder = Dictionary::builder()
        .chars(chars)
        .mode(config.effective_mode());
    if let Some(p) = padding {
        builder = builder.padding(p);
    }
    let dictionary = builder
        .build()
        .map_err(|e| WasmError::from(format!("Failed to build dictionary: {:?}", e)))?;

    decode_internal(encoded, &dictionary).map_err(WasmError::from)
}

/// Encode bytes using a specified dictionary
///
/// # Arguments
///
/// * `data` - The binary data to encode
/// * `dictionary_name` - Name of the dictionary to use (e.g., "base64", "emoji", "hieroglyphs")
///
/// # Returns
///
/// Encoded string
#[wasm_bindgen]
pub fn encode_with_dictionary(data: &[u8], dictionary_name: &str) -> Result<String, WasmError> {
    let registry = DictionaryRegistry::load_default()
        .map_err(|e| WasmError::from(format!("Failed to load registry: {:?}", e)))?;

    let config = registry
        .get_dictionary(dictionary_name)
        .ok_or_else(|| WasmError::from(format!("Dictionary '{}' not found", dictionary_name)))?;

    let chars: Vec<char> = config.chars.chars().collect();
    let padding = config.padding.as_ref().and_then(|s| s.chars().next());
    let mut builder = Dictionary::builder()
        .chars(chars)
        .mode(config.effective_mode());
    if let Some(p) = padding {
        builder = builder.padding(p);
    }
    let dictionary = builder
        .build()
        .map_err(|e| WasmError::from(format!("Failed to build dictionary: {:?}", e)))?;

    Ok(encode_internal(data, &dictionary))
}

/// Decode a string using a specified dictionary
///
/// # Arguments
///
/// * `encoded` - The encoded string
/// * `dictionary_name` - Name of the dictionary that was used for encoding
///
/// # Returns
///
/// Decoded binary data
#[wasm_bindgen]
pub fn decode_with_dictionary(encoded: &str, dictionary_name: &str) -> Result<Vec<u8>, WasmError> {
    let registry = DictionaryRegistry::load_default()
        .map_err(|e| WasmError::from(format!("Failed to load registry: {:?}", e)))?;

    let config = registry
        .get_dictionary(dictionary_name)
        .ok_or_else(|| WasmError::from(format!("Dictionary '{}' not found", dictionary_name)))?;

    let chars: Vec<char> = config.chars.chars().collect();
    let padding = config.padding.as_ref().and_then(|s| s.chars().next());
    let mut builder = Dictionary::builder()
        .chars(chars)
        .mode(config.effective_mode());
    if let Some(p) = padding {
        builder = builder.padding(p);
    }
    let dictionary = builder
        .build()
        .map_err(|e| WasmError::from(format!("Failed to build dictionary: {:?}", e)))?;

    decode_internal(encoded, &dictionary).map_err(WasmError::from)
}

/// List all available built-in dictionaries
///
/// # Returns
///
/// Array of dictionary names
#[wasm_bindgen]
pub fn list_dictionaries() -> Result<Vec<String>, WasmError> {
    let registry = DictionaryRegistry::load_default()
        .map_err(|e| WasmError::from(format!("Failed to load registry: {:?}", e)))?;

    Ok(registry
        .dictionaries
        .keys()
        .map(|s: &String| s.to_string())
        .collect())
}
