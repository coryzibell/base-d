pub mod binary_packer;
pub mod binary_unpacker;
pub mod compression;
pub mod display96;
pub mod frame;
pub mod parsers;
pub mod serializers;
pub mod stele;
pub mod stele_analyzer;
pub mod types;

#[cfg(test)]
mod edge_cases;

// Re-export key types for convenience
pub use binary_packer::pack;
pub use binary_unpacker::unpack;
pub use compression::SchemaCompressionAlgo;
pub use frame::{decode_framed, encode_framed};
pub use parsers::{InputParser, JsonParser};
// MarkdownDocParser used internally by encode_markdown_stele_* functions
pub use serializers::{JsonSerializer, OutputSerializer};
pub use types::{
    FieldDef, FieldType, IntermediateRepresentation, SchemaError, SchemaHeader, SchemaValue,
};

// Re-export stele functions for library users
#[allow(unused_imports)]
pub use stele::{parse as parse_stele, serialize as serialize_stele};

/// Encode JSON to schema format: JSON → IR → binary → \[compress\] → display96 → framed
///
/// Transforms JSON into a compact, display-safe wire format suitable for LLM-to-LLM communication.
/// The output is wrapped in Egyptian hieroglyph delimiters (`𓍹...𓍺`) and uses a 96-character
/// alphabet of box-drawing and geometric shapes.
///
/// # Arguments
///
/// * `json` - JSON string to encode (must be object or array of objects)
/// * `compress` - Optional compression algorithm (brotli, lz4, or zstd)
///
/// # Returns
///
/// Returns a framed, display-safe string like `𓍹{encoded_payload}𓍺`
///
/// # Errors
///
/// * `SchemaError::InvalidInput` - Invalid JSON or unsupported structure (e.g., root primitives)
/// * `SchemaError::Compression` - Compression failure
///
/// # Example
///
/// ```ignore
/// use base_d::{encode_schema, SchemaCompressionAlgo};
///
/// let json = r#"{"users":[{"id":1,"name":"alice"}]}"#;
///
/// // Without compression
/// let encoded = encode_schema(json, None)?;
/// println!("{}", encoded); // 𓍹╣◟╥◕◝▰◣◥▟╺▖◘▰◝▤◀╧𓍺
///
/// // With brotli compression
/// let compressed = encode_schema(json, Some(SchemaCompressionAlgo::Brotli))?;
/// ```
///
/// # See Also
///
/// * [`decode_schema`] - Decode schema format back to JSON
/// * [SCHEMA.md](../../../SCHEMA.md) - Full format specification
pub fn encode_schema(
    json: &str,
    compress: Option<SchemaCompressionAlgo>,
) -> Result<String, SchemaError> {
    use parsers::{InputParser, JsonParser};

    let ir = JsonParser::parse(json)?;
    let binary = pack(&ir);
    let compressed = compression::compress_with_prefix(&binary, compress)?;
    Ok(frame::encode_framed(&compressed))
}

/// Decode schema format to JSON: framed → display96 → \[decompress\] → binary → IR → JSON
///
/// Reverses the schema encoding pipeline to reconstruct the original JSON from the framed,
/// display-safe wire format. Automatically detects and handles compression.
///
/// # Arguments
///
/// * `encoded` - Schema-encoded string with delimiters (`𓍹...𓍺`)
/// * `pretty` - Pretty-print JSON output with indentation
///
/// # Returns
///
/// Returns the decoded JSON string (minified or pretty-printed)
///
/// # Errors
///
/// * `SchemaError::InvalidFrame` - Missing or invalid frame delimiters
/// * `SchemaError::InvalidCharacter` - Invalid character in display96 payload
/// * `SchemaError::Decompression` - Decompression failure
/// * `SchemaError::UnexpectedEndOfData` - Truncated or corrupted binary data
/// * `SchemaError::InvalidTypeTag` - Invalid type tag in header
///
/// # Example
///
/// ```ignore
/// use base_d::decode_schema;
///
/// let encoded = "𓍹╣◟╥◕◝▰◣◥▟╺▖◘▰◝▤◀╧𓍺";
///
/// // Minified output
/// let json = decode_schema(encoded, false)?;
/// println!("{}", json); // {"users":[{"id":1,"name":"alice"}]}
///
/// // Pretty-printed output
/// let pretty = decode_schema(encoded, true)?;
/// println!("{}", pretty);
/// // {
/// //   "users": [
/// //     {"id": 1, "name": "alice"}
/// //   ]
/// // }
/// ```
///
/// # See Also
///
/// * [`encode_schema`] - Encode JSON to schema format
/// * [SCHEMA.md](../../../SCHEMA.md) - Full format specification
pub fn decode_schema(encoded: &str, pretty: bool) -> Result<String, SchemaError> {
    use serializers::{JsonSerializer, OutputSerializer};

    let compressed = frame::decode_framed(encoded)?;
    let binary = compression::decompress_with_prefix(&compressed)?;
    let ir = unpack(&binary)?;
    JsonSerializer::serialize(&ir, pretty)
}

/// Encode JSON to stele format: JSON → IR → stele
///
/// Transforms JSON into a model-readable structured format using Unicode delimiters.
/// Unlike carrier98 (opaque binary), stele is designed for models to parse directly.
///
/// # Format
///
/// ```text
/// @{root}┃{field}:{type}┃{field}:{type}...
/// ◉{value}┃{value}┃{value}...
/// ```
///
/// # Example
///
/// ```ignore
/// use base_d::encode_stele;
///
/// let json = r#"{"users":[{"id":1,"name":"alice"}]}"#;
/// let stele = encode_stele(json)?;
/// // @users┃id:int┃name:str
/// // ◉1┃alice
/// ```
pub fn encode_stele(json: &str, minify: bool) -> Result<String, SchemaError> {
    encode_stele_with_options(json, minify, true, true)
}

pub fn encode_stele_minified(json: &str) -> Result<String, SchemaError> {
    encode_stele_with_options(json, true, true, true)
}

/// Encode JSON to stele without tokenization (human-readable field names)
pub fn encode_stele_readable(json: &str, minify: bool) -> Result<String, SchemaError> {
    encode_stele_with_options(json, minify, false, false)
}

/// Encode JSON to stele with field tokenization only (no value dictionary)
pub fn encode_stele_light(json: &str, minify: bool) -> Result<String, SchemaError> {
    encode_stele_with_options(json, minify, true, false)
}

/// Encode JSON to stele path mode (one line per leaf value)
pub fn encode_stele_path(json: &str) -> Result<String, SchemaError> {
    stele::serialize_path_mode(json)
}

/// Decode stele path mode to JSON
pub fn decode_stele_path(path_input: &str) -> Result<String, SchemaError> {
    stele::parse_path_mode(path_input)
}

/// Encode JSON to ASCII inline stele format
pub fn encode_stele_ascii(json: &str) -> Result<String, SchemaError> {
    use parsers::{InputParser, JsonParser};
    let ir = JsonParser::parse(json)?;
    stele::serialize_ascii(&ir)
}

/// Encode markdown document to ASCII inline stele format
pub fn encode_markdown_stele_ascii(markdown: &str) -> Result<String, SchemaError> {
    use parsers::{InputParser, MarkdownDocParser};
    let ir = MarkdownDocParser::parse(markdown)?;
    stele::serialize_ascii(&ir)
}

/// Encode markdown document to markdown-like inline stele format
/// Uses #1-#6 for headers, -1/-2 for lists, preserves markdown syntax patterns
pub fn encode_markdown_stele_markdown(markdown: &str) -> Result<String, SchemaError> {
    use parsers::{InputParser, MarkdownDocParser};
    let ir = MarkdownDocParser::parse(markdown)?;
    stele::serialize_markdown(&ir)
}

/// Encode markdown document to stele format: markdown → IR → stele
///
/// Parses a full markdown document into a simplified block-based representation,
/// then encodes to stele format for model-readable output.
pub fn encode_markdown_stele(markdown: &str, minify: bool) -> Result<String, SchemaError> {
    encode_markdown_stele_with_options(markdown, minify, true, true)
}

/// Encode markdown to stele without tokenization (human-readable)
pub fn encode_markdown_stele_readable(markdown: &str, minify: bool) -> Result<String, SchemaError> {
    encode_markdown_stele_with_options(markdown, minify, false, false)
}

/// Encode markdown to stele with field tokenization only (no value dictionary)
pub fn encode_markdown_stele_light(markdown: &str, minify: bool) -> Result<String, SchemaError> {
    encode_markdown_stele_with_options(markdown, minify, true, false)
}

fn encode_markdown_stele_with_options(
    markdown: &str,
    minify: bool,
    tokenize_fields: bool,
    tokenize_values: bool,
) -> Result<String, SchemaError> {
    use parsers::{InputParser, MarkdownDocParser};

    let ir = MarkdownDocParser::parse(markdown)?;
    match (tokenize_fields, tokenize_values) {
        (true, true) => stele::serialize(&ir, minify),
        (true, false) => stele::serialize_light(&ir, minify),
        (false, false) => stele::serialize_readable(&ir, minify),
        (false, true) => {
            // Invalid: can't tokenize values without tokenizing fields
            stele::serialize_readable(&ir, minify)
        }
    }
}

fn encode_stele_with_options(
    json: &str,
    minify: bool,
    tokenize_fields: bool,
    tokenize_values: bool,
) -> Result<String, SchemaError> {
    use parsers::{InputParser, JsonParser};

    let ir = JsonParser::parse(json)?;
    match (tokenize_fields, tokenize_values) {
        (true, true) => stele::serialize(&ir, minify),
        (true, false) => stele::serialize_light(&ir, minify),
        (false, false) => stele::serialize_readable(&ir, minify),
        (false, true) => {
            // Invalid: can't tokenize values without tokenizing fields
            stele::serialize_readable(&ir, minify)
        }
    }
}

/// Decode stele format to JSON: stele → IR → JSON
///
/// Reverses the stele encoding to reconstruct JSON from the model-readable format.
///
/// # Example
///
/// ```ignore
/// use base_d::decode_stele;
///
/// let stele = "@users┃id:int┃name:str\n◉1┃alice";
/// let json = decode_stele(stele, false)?;
/// // {"users":[{"id":1,"name":"alice"}]}
/// ```
pub fn decode_stele(stele_input: &str, pretty: bool) -> Result<String, SchemaError> {
    use serializers::{JsonSerializer, OutputSerializer};

    let ir = stele::parse(stele_input)?;
    JsonSerializer::serialize(&ir, pretty)
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::encoders::algorithms::schema::types::{
        FLAG_HAS_NULLS, FLAG_HAS_ROOT_KEY, FieldDef, FieldType, IntermediateRepresentation,
        SchemaHeader, SchemaValue,
    };
    use parsers::{InputParser, JsonParser};
    use serializers::{JsonSerializer, OutputSerializer};

    #[test]
    fn test_round_trip_simple() {
        let fields = vec![
            FieldDef::new("id", FieldType::U64),
            FieldDef::new("name", FieldType::String),
        ];
        let header = SchemaHeader::new(2, fields);

        let values = vec![
            SchemaValue::U64(1),
            SchemaValue::String("Alice".to_string()),
            SchemaValue::U64(2),
            SchemaValue::String("Bob".to_string()),
        ];

        let original = IntermediateRepresentation::new(header, values).unwrap();

        // Pack and unpack
        let packed = pack(&original);
        let unpacked = unpack(&packed).unwrap();

        assert_eq!(original, unpacked);
    }

    #[test]
    fn test_round_trip_all_types() {
        let fields = vec![
            FieldDef::new("u64_field", FieldType::U64),
            FieldDef::new("i64_field", FieldType::I64),
            FieldDef::new("f64_field", FieldType::F64),
            FieldDef::new("string_field", FieldType::String),
            FieldDef::new("bool_field", FieldType::Bool),
        ];
        let header = SchemaHeader::new(1, fields);

        let values = vec![
            SchemaValue::U64(42),
            SchemaValue::I64(-42),
            SchemaValue::F64(std::f64::consts::PI),
            SchemaValue::String("test".to_string()),
            SchemaValue::Bool(true),
        ];

        let original = IntermediateRepresentation::new(header, values).unwrap();

        let packed = pack(&original);
        let unpacked = unpack(&packed).unwrap();

        assert_eq!(original, unpacked);
    }

    #[test]
    fn test_round_trip_with_root_key() {
        let mut header = SchemaHeader::new(1, vec![FieldDef::new("id", FieldType::U64)]);
        header.root_key = Some("users".to_string());
        header.set_flag(FLAG_HAS_ROOT_KEY);

        let values = vec![SchemaValue::U64(42)];
        let original = IntermediateRepresentation::new(header, values).unwrap();

        let packed = pack(&original);
        let unpacked = unpack(&packed).unwrap();

        assert_eq!(original, unpacked);
    }

    #[test]
    fn test_round_trip_with_nulls() {
        let mut header = SchemaHeader::new(
            2,
            vec![
                FieldDef::new("id", FieldType::U64),
                FieldDef::new("name", FieldType::String),
            ],
        );

        // Mark second value as null (row 0, field 1)
        let total_values: usize = 2 * 2; // 2 rows * 2 fields = 4 values
        let bitmap_bytes = total_values.div_ceil(8); // 1 byte
        let mut null_bitmap = vec![0u8; bitmap_bytes];
        null_bitmap[0] |= 1 << 1; // Set bit 1 (second value)

        header.null_bitmap = Some(null_bitmap);
        header.set_flag(FLAG_HAS_NULLS);

        let values = vec![
            SchemaValue::U64(1),
            SchemaValue::Null, // This is marked as null in bitmap
            SchemaValue::U64(2),
            SchemaValue::String("Bob".to_string()),
        ];

        let original = IntermediateRepresentation::new(header, values).unwrap();

        let packed = pack(&original);
        let unpacked = unpack(&packed).unwrap();

        assert_eq!(original, unpacked);
    }

    #[test]
    fn test_round_trip_array() {
        let fields = vec![FieldDef::new(
            "tags",
            FieldType::Array(Box::new(FieldType::U64)),
        )];
        let header = SchemaHeader::new(1, fields);

        let values = vec![SchemaValue::Array(vec![
            SchemaValue::U64(1),
            SchemaValue::U64(2),
            SchemaValue::U64(3),
        ])];

        let original = IntermediateRepresentation::new(header, values).unwrap();

        let packed = pack(&original);
        let unpacked = unpack(&packed).unwrap();

        assert_eq!(original, unpacked);
    }

    #[test]
    fn test_round_trip_large_values() {
        let fields = vec![
            FieldDef::new("large_u64", FieldType::U64),
            FieldDef::new("large_i64", FieldType::I64),
        ];
        let header = SchemaHeader::new(1, fields);

        let values = vec![SchemaValue::U64(u64::MAX), SchemaValue::I64(i64::MIN)];

        let original = IntermediateRepresentation::new(header, values).unwrap();

        let packed = pack(&original);
        let unpacked = unpack(&packed).unwrap();

        assert_eq!(original, unpacked);
    }

    #[test]
    fn test_round_trip_empty_string() {
        let fields = vec![FieldDef::new("name", FieldType::String)];
        let header = SchemaHeader::new(1, fields);

        let values = vec![SchemaValue::String("".to_string())];

        let original = IntermediateRepresentation::new(header, values).unwrap();

        let packed = pack(&original);
        let unpacked = unpack(&packed).unwrap();

        assert_eq!(original, unpacked);
    }

    #[test]
    fn test_round_trip_multiple_rows() {
        let fields = vec![
            FieldDef::new("id", FieldType::U64),
            FieldDef::new("score", FieldType::F64),
            FieldDef::new("active", FieldType::Bool),
        ];
        let header = SchemaHeader::new(3, fields);

        let values = vec![
            SchemaValue::U64(1),
            SchemaValue::F64(95.5),
            SchemaValue::Bool(true),
            SchemaValue::U64(2),
            SchemaValue::F64(87.3),
            SchemaValue::Bool(false),
            SchemaValue::U64(3),
            SchemaValue::F64(92.1),
            SchemaValue::Bool(true),
        ];

        let original = IntermediateRepresentation::new(header, values).unwrap();

        let packed = pack(&original);
        let unpacked = unpack(&packed).unwrap();

        assert_eq!(original, unpacked);
    }

    #[test]
    fn test_invalid_data() {
        // Empty data
        let result = unpack(&[]);
        assert!(matches!(
            result,
            Err(SchemaError::UnexpectedEndOfData { .. })
        ));

        // Truncated data
        let result = unpack(&[0, 1, 2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_full_roundtrip() {
        let input = r#"{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]}"#;
        let ir = JsonParser::parse(input).unwrap();
        let binary = pack(&ir);
        let compressed = compression::compress_with_prefix(&binary, None).unwrap();
        let decompressed = compression::decompress_with_prefix(&compressed).unwrap();
        let ir2 = unpack(&decompressed).unwrap();
        let output = JsonSerializer::serialize(&ir2, false).unwrap();

        // Parse both as serde_json::Value and compare (order-independent)
        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_json_simple_object() {
        let input = r#"{"id":1,"name":"alice","score":95.5}"#;
        let ir = JsonParser::parse(input).unwrap();
        let binary = pack(&ir);
        let ir2 = unpack(&binary).unwrap();
        let output = JsonSerializer::serialize(&ir2, false).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_json_swapi_nested_arrays() {
        // SWAPI-like data with nested arrays of primitives
        // Primitive arrays now stored inline
        let input = r#"{"people":[{"name":"Luke","height":"172","films":["film/1","film/2"],"vehicles":[]},{"name":"C-3PO","height":"167","films":["film/1","film/2","film/3"],"vehicles":[]}]}"#;
        let ir = JsonParser::parse(input).unwrap();

        // Verify stele representation (readable mode for string matching)
        let stele_output = stele::serialize_readable(&ir, false).unwrap();

        // Should have @people root key
        assert!(stele_output.starts_with("@people"));
        // Primitive arrays now inline with superscript + ⟦⟧ syntax
        assert!(stele_output.contains("filmsˢ⟦⟧"));
        assert!(stele_output.contains("vehiclesˢ⟦⟧"));

        // Verify round trip - arrays become indexed objects
        let binary = pack(&ir);
        let ir2 = unpack(&binary).unwrap();
        let output = JsonSerializer::serialize(&ir2, false).unwrap();

        // Parse output and verify structure
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        let people = output_value
            .as_object()
            .unwrap()
            .get("people")
            .unwrap()
            .as_array()
            .unwrap();

        // First person has films as properly reconstructed array
        let luke = &people[0];
        assert_eq!(luke["name"], "Luke");
        assert_eq!(luke["height"], "172");
        let luke_films = luke["films"].as_array().unwrap();
        assert_eq!(luke_films[0], "film/1");
        assert_eq!(luke_films[1], "film/2");
    }

    #[test]
    fn test_json_wrapper_keys() {
        // Test common pagination wrapper keys get unwrapped
        let test_cases = vec![
            r#"{"results":[{"id":1,"name":"a"},{"id":2,"name":"b"}]}"#,
            r#"{"data":[{"id":1,"name":"a"},{"id":2,"name":"b"}]}"#,
            r#"{"items":[{"id":1,"name":"a"},{"id":2,"name":"b"}]}"#,
            r#"{"records":[{"id":1,"name":"a"},{"id":2,"name":"b"}]}"#,
        ];

        for input in test_cases {
            let ir = JsonParser::parse(input).unwrap();

            // Should have root key from wrapper
            assert!(ir.header.root_key.is_some());
            let root = ir.header.root_key.as_ref().unwrap();
            assert!(root == "results" || root == "data" || root == "items" || root == "records");

            // Should have 2 rows (unwrapped the array)
            assert_eq!(ir.header.row_count, 2);

            // Round trip should preserve data
            let binary = pack(&ir);
            let ir2 = unpack(&binary).unwrap();
            let output = JsonSerializer::serialize(&ir2, false).unwrap();

            let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
            let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
            assert_eq!(input_value, output_value);
        }
    }

    #[test]
    fn test_json_nested_objects() {
        let input = r#"{"user":{"profile":{"name":"alice","age":30}}}"#;
        let ir = JsonParser::parse(input).unwrap();
        let binary = pack(&ir);
        let ir2 = unpack(&binary).unwrap();
        let output = JsonSerializer::serialize(&ir2, false).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_json_with_nulls() {
        let input = r#"{"name":"alice","age":null,"active":true}"#;
        let ir = JsonParser::parse(input).unwrap();
        assert!(ir.header.has_flag(FLAG_HAS_NULLS));

        let binary = pack(&ir);
        let ir2 = unpack(&binary).unwrap();
        let output = JsonSerializer::serialize(&ir2, false).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_json_with_arrays() {
        // Arrays now flatten to indexed objects
        let input = r#"{"scores":[95,87,92],"tags":["rust","json"]}"#;
        let ir = JsonParser::parse(input).unwrap();
        let binary = pack(&ir);
        let ir2 = unpack(&binary).unwrap();
        let output = JsonSerializer::serialize(&ir2, false).unwrap();

        // Expected: arrays are properly reconstructed as arrays
        let expected = r#"{"scores":[95,87,92],"tags":["rust","json"]}"#;
        let expected_value: serde_json::Value = serde_json::from_str(expected).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(expected_value, output_value);
    }

    #[test]
    fn test_encode_schema_roundtrip() {
        let input = r#"{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]}"#;
        let encoded = encode_schema(input, None).unwrap();

        // Validate frame delimiters
        assert!(encoded.starts_with(frame::FRAME_START));
        assert!(encoded.ends_with(frame::FRAME_END));

        // Decode back to JSON
        let decoded = decode_schema(&encoded, false).unwrap();

        // Compare as JSON values (order-independent)
        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_encode_schema_simple() {
        let input = r#"{"id":1,"name":"alice","score":95.5}"#;
        let encoded = encode_schema(input, None).unwrap();
        let decoded = decode_schema(&encoded, false).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_encode_schema_with_nulls() {
        let input = r#"{"name":"alice","age":null,"active":true}"#;
        let encoded = encode_schema(input, None).unwrap();
        let decoded = decode_schema(&encoded, false).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_encode_schema_empty_object() {
        let input = r#"{}"#;
        let result = encode_schema(input, None);
        // Empty objects should fail or handle gracefully
        // This depends on JsonParser behavior
        println!("Empty object result: {:?}", result);
    }

    #[test]
    fn test_decode_schema_invalid_frame() {
        let invalid = "not_framed_data";
        let result = decode_schema(invalid, false);
        assert!(matches!(result, Err(SchemaError::InvalidFrame(_))));
    }

    #[test]
    fn test_decode_schema_invalid_chars() {
        let invalid = format!("{}ABC{}", frame::FRAME_START, frame::FRAME_END);
        let result = decode_schema(&invalid, false);
        assert!(matches!(result, Err(SchemaError::InvalidCharacter(_))));
    }

    #[test]
    fn test_visual_wire_format() {
        let input = r#"{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]}"#;
        let encoded = encode_schema(input, None).unwrap();

        println!("\n=== Visual Wire Format ===");
        println!("Input JSON: {}", input);
        println!("Input length: {} bytes", input.len());
        println!("\nEncoded output: {}", encoded);
        println!(
            "Encoded length: {} chars ({} bytes UTF-8)",
            encoded.chars().count(),
            encoded.len()
        );

        // Calculate compression ratio
        let compression_ratio = input.len() as f64 / encoded.len() as f64;
        println!("Compression ratio: {:.2}x", compression_ratio);

        // Decode and verify
        let decoded = decode_schema(&encoded, false).unwrap();
        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(input_value, output_value);
        println!("Roundtrip verified ✓\n");
    }

    #[test]
    fn test_compression_comparison() {
        let test_cases = [
            r#"{"id":1}"#,
            r#"{"id":1,"name":"alice"}"#,
            r#"{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]}"#,
            r#"{"data":[1,2,3,4,5,6,7,8,9,10]}"#,
        ];

        println!("\n=== Compression Comparison ===");
        for (i, input) in test_cases.iter().enumerate() {
            let encoded = encode_schema(input, None).unwrap();
            let ratio = input.len() as f64 / encoded.len() as f64;

            println!(
                "Test case {}: {} bytes → {} bytes ({:.2}x)",
                i + 1,
                input.len(),
                encoded.len(),
                ratio
            );
        }
        println!();
    }

    #[test]
    fn test_encode_schema_with_compression() {
        use super::SchemaCompressionAlgo;

        let input = r#"{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"},{"id":3,"name":"charlie"}]}"#;

        // Test each compression algorithm
        for algo in [
            SchemaCompressionAlgo::Brotli,
            SchemaCompressionAlgo::Lz4,
            SchemaCompressionAlgo::Zstd,
        ] {
            let encoded = encode_schema(input, Some(algo)).unwrap();
            let decoded = decode_schema(&encoded, false).unwrap();

            let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
            let output_value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
            assert_eq!(
                input_value, output_value,
                "Failed for compression algorithm: {:?}",
                algo
            );
        }
    }

    #[test]
    fn test_compression_size_comparison() {
        use super::SchemaCompressionAlgo;

        let input = r#"{"users":[{"id":1,"name":"alice","active":true,"score":95.5},{"id":2,"name":"bob","active":false,"score":87.3},{"id":3,"name":"charlie","active":true,"score":92.1}]}"#;

        println!("\n=== Compression Size Comparison ===");
        println!("Input JSON: {} bytes", input.len());

        let no_compress = encode_schema(input, None).unwrap();
        println!("No compression: {} bytes", no_compress.len());

        for algo in [
            SchemaCompressionAlgo::Brotli,
            SchemaCompressionAlgo::Lz4,
            SchemaCompressionAlgo::Zstd,
        ] {
            let compressed = encode_schema(input, Some(algo)).unwrap();
            let ratio = no_compress.len() as f64 / compressed.len() as f64;
            println!(
                "{:?}: {} bytes ({:.2}x vs uncompressed)",
                algo,
                compressed.len(),
                ratio
            );
        }
        println!();
    }

    #[test]
    fn test_nested_object_roundtrip_single_level() {
        let input = r#"{"id":"A1","name":"Jim","grade":{"math":60,"physics":66,"chemistry":61}}"#;

        // JSON → IR → stele (readable for string matching)
        let ir = JsonParser::parse(input).unwrap();
        let stele = stele::serialize_readable(&ir, false).unwrap();

        // Verify flattened field names with ჻ and superscript types
        assert!(stele.contains("grade჻mathⁱ"));
        assert!(stele.contains("grade჻physicsⁱ"));
        assert!(stele.contains("grade჻chemistryⁱ"));

        // stele → IR → JSON (using tokenized format for roundtrip)
        let tokenized = stele::serialize(&ir, false).unwrap();
        let ir2 = stele::parse(&tokenized).unwrap();
        let output = JsonSerializer::serialize(&ir2, false).unwrap();

        // Compare JSON
        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_nested_object_roundtrip_deep() {
        let input = r#"{"a":{"b":{"c":{"d":42}}}}"#;

        let ir = JsonParser::parse(input).unwrap();
        let stele = stele::serialize_readable(&ir, false).unwrap();

        // Verify deep nesting with ჻ and superscript type
        assert!(stele.contains("a჻b჻c჻dⁱ"));

        // Roundtrip with tokenized format
        let tokenized = stele::serialize(&ir, false).unwrap();
        let ir2 = stele::parse(&tokenized).unwrap();
        let output = JsonSerializer::serialize(&ir2, false).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_nested_object_roundtrip_array_of_objects() {
        let input = r#"{"students":[{"id":"A1","name":"Jim","grade":{"math":60,"physics":66}},{"id":"B2","name":"Sara","grade":{"math":85,"physics":90}}]}"#;

        let ir = JsonParser::parse(input).unwrap();
        let stele = stele::serialize_readable(&ir, false).unwrap();

        // Verify root key and flattened nested fields with superscript types
        assert!(stele.starts_with("@students"));
        assert!(stele.contains("grade჻mathⁱ"));
        assert!(stele.contains("grade჻physicsⁱ"));

        // Roundtrip with tokenized format
        let tokenized = stele::serialize(&ir, false).unwrap();
        let ir2 = stele::parse(&tokenized).unwrap();
        let output = JsonSerializer::serialize(&ir2, false).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_nested_object_roundtrip_mixed_with_arrays() {
        // Primitive arrays now stored inline
        let input = r#"{"person":{"name":"Alice","tags":["admin","user"],"address":{"city":"Boston","zip":"02101"}}}"#;

        let ir = JsonParser::parse(input).unwrap();
        let stele = stele::serialize_readable(&ir, false).unwrap();

        // Verify both object nesting and inline primitive arrays with superscript types
        assert!(stele.contains("person჻nameˢ"));
        // Primitive arrays now inline with superscript + ⟦⟧ syntax
        assert!(stele.contains("person჻tagsˢ⟦⟧"));
        assert!(stele.contains("person჻address჻cityˢ"));
        assert!(stele.contains("person჻address჻zipˢ"));

        // Roundtrip with tokenized format
        let tokenized = stele::serialize(&ir, false).unwrap();
        let ir2 = stele::parse(&tokenized).unwrap();
        let output = JsonSerializer::serialize(&ir2, false).unwrap();

        // Arrays are properly reconstructed
        let expected = r#"{"person":{"address":{"city":"Boston","zip":"02101"},"name":"Alice","tags":["admin","user"]}}"#;
        let expected_value: serde_json::Value = serde_json::from_str(expected).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(expected_value, output_value);
    }

    #[test]
    fn test_nested_object_roundtrip_schema_encode() {
        let input = r#"{"data":{"user":{"profile":{"name":"alice","age":30}}}}"#;

        // Full schema pipeline: JSON → IR → binary → display96 → framed
        let encoded = encode_schema(input, None).unwrap();
        let decoded = decode_schema(&encoded, false).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(input_value, output_value);
    }
}
