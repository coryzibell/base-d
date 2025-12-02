pub mod binary_packer;
pub mod binary_unpacker;
pub mod display96;
pub mod frame;
pub mod parsers;
pub mod serializers;
pub mod types;

// Re-export key types for convenience
pub use binary_packer::pack;
pub use binary_unpacker::unpack;
pub use types::SchemaError;

/// Full schema encoding pipeline: JSON → IR → binary → display96 → framed
///
/// # Example
/// ```ignore
/// let json = r#"{"users":[{"id":1,"name":"alice"}]}"#;
/// let encoded = encode_schema(json)?;
/// // Returns: "𓍹{display96_encoded}𓍺"
/// ```
pub fn encode_schema(json: &str) -> Result<String, SchemaError> {
    use parsers::{InputParser, JsonParser};

    let ir = JsonParser::parse(json)?;
    let binary = pack(&ir);
    Ok(frame::encode_framed(&binary))
}

/// Full schema decoding pipeline: framed → display96 → binary → IR → JSON
///
/// # Example
/// ```ignore
/// let framed = "𓍹{display96_encoded}𓍺";
/// let json = decode_schema(framed)?;
/// ```
pub fn decode_schema(encoded: &str) -> Result<String, SchemaError> {
    use serializers::{JsonSerializer, OutputSerializer};

    let binary = frame::decode_framed(encoded)?;
    let ir = unpack(&binary)?;
    JsonSerializer::serialize(&ir)
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
        assert!(matches!(result, Err(SchemaError::UnexpectedEndOfData)));

        // Truncated data
        let result = unpack(&[0, 1, 2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_full_roundtrip() {
        let input = r#"{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]}"#;
        let ir = JsonParser::parse(input).unwrap();
        let binary = pack(&ir);
        let ir2 = unpack(&binary).unwrap();
        let output = JsonSerializer::serialize(&ir2).unwrap();

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
        let output = JsonSerializer::serialize(&ir2).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_json_nested_objects() {
        let input = r#"{"user":{"profile":{"name":"alice","age":30}}}"#;
        let ir = JsonParser::parse(input).unwrap();
        let binary = pack(&ir);
        let ir2 = unpack(&binary).unwrap();
        let output = JsonSerializer::serialize(&ir2).unwrap();

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
        let output = JsonSerializer::serialize(&ir2).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_json_with_arrays() {
        let input = r#"{"scores":[95,87,92],"tags":["rust","json"]}"#;
        let ir = JsonParser::parse(input).unwrap();
        let binary = pack(&ir);
        let ir2 = unpack(&binary).unwrap();
        let output = JsonSerializer::serialize(&ir2).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_encode_schema_roundtrip() {
        let input = r#"{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]}"#;
        let encoded = encode_schema(input).unwrap();

        // Validate frame delimiters
        assert!(encoded.starts_with(frame::FRAME_START));
        assert!(encoded.ends_with(frame::FRAME_END));

        // Decode back to JSON
        let decoded = decode_schema(&encoded).unwrap();

        // Compare as JSON values (order-independent)
        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_encode_schema_simple() {
        let input = r#"{"id":1,"name":"alice","score":95.5}"#;
        let encoded = encode_schema(input).unwrap();
        let decoded = decode_schema(&encoded).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_encode_schema_with_nulls() {
        let input = r#"{"name":"alice","age":null,"active":true}"#;
        let encoded = encode_schema(input).unwrap();
        let decoded = decode_schema(&encoded).unwrap();

        let input_value: serde_json::Value = serde_json::from_str(input).unwrap();
        let output_value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(input_value, output_value);
    }

    #[test]
    fn test_encode_schema_empty_object() {
        let input = r#"{}"#;
        let result = encode_schema(input);
        // Empty objects should fail or handle gracefully
        // This depends on JsonParser behavior
        println!("Empty object result: {:?}", result);
    }

    #[test]
    fn test_decode_schema_invalid_frame() {
        let invalid = "not_framed_data";
        let result = decode_schema(invalid);
        assert!(matches!(result, Err(SchemaError::InvalidFrame(_))));
    }

    #[test]
    fn test_decode_schema_invalid_chars() {
        let invalid = format!("{}ABC{}", frame::FRAME_START, frame::FRAME_END);
        let result = decode_schema(&invalid);
        assert!(matches!(result, Err(SchemaError::InvalidCharacter(_))));
    }

    #[test]
    fn test_visual_wire_format() {
        let input = r#"{"users":[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]}"#;
        let encoded = encode_schema(input).unwrap();

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
        let decoded = decode_schema(&encoded).unwrap();
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
            let encoded = encode_schema(input).unwrap();
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
}
