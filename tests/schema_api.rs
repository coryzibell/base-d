/// Tests for the public schema API
///
/// Verifies that all schema IR types and traits are properly exposed
/// for library users to build custom frontends.

#[test]
fn test_schema_types_are_accessible() {
    // Verify all types are accessible through the public API
    use base_d::schema::{
        FieldDef, FieldType, IntermediateRepresentation, SchemaHeader, SchemaValue,
    };

    // Create IR from scratch
    let fields = vec![
        FieldDef::new("id", FieldType::U64),
        FieldDef::new("name", FieldType::String),
    ];

    let header = SchemaHeader::new(2, fields);

    let values = vec![
        SchemaValue::U64(1),
        SchemaValue::String("alice".to_string()),
        SchemaValue::U64(2),
        SchemaValue::String("bob".to_string()),
    ];

    let ir = IntermediateRepresentation::new(header, values).expect("Failed to create IR");

    assert_eq!(ir.header.row_count, 2);
    assert_eq!(ir.header.fields.len(), 2);
    assert_eq!(ir.values.len(), 4);
}

#[test]
fn test_binary_layer_functions() {
    use base_d::schema::{
        FieldDef, FieldType, IntermediateRepresentation, SchemaHeader, SchemaValue, decode_framed,
        encode_framed, pack, unpack,
    };

    // Create IR
    let fields = vec![FieldDef::new("value", FieldType::U64)];
    let header = SchemaHeader::new(1, fields);
    let values = vec![SchemaValue::U64(42)];
    let ir = IntermediateRepresentation::new(header, values).expect("Failed to create IR");

    // Pack to binary
    let binary = pack(&ir);
    assert!(!binary.is_empty());

    // Unpack from binary
    let ir2 = unpack(&binary).expect("Failed to unpack");
    assert_eq!(ir, ir2);

    // Frame encoding
    let framed = encode_framed(&binary);
    assert!(framed.starts_with('𓍹'));
    assert!(framed.ends_with('𓍺'));

    // Frame decoding
    let decoded = decode_framed(&framed).expect("Failed to decode frame");
    assert_eq!(binary, decoded);
}

#[test]
fn test_custom_parser_implementation() {
    use base_d::schema::{
        FieldDef, FieldType, InputParser, IntermediateRepresentation, SchemaError, SchemaHeader,
        SchemaValue, encode_framed, pack,
    };

    // Define a minimal custom parser
    struct SimpleParser;

    impl InputParser for SimpleParser {
        type Error = SchemaError;

        fn parse(input: &str) -> Result<IntermediateRepresentation, Self::Error> {
            // Parse comma-separated values: "field1=value1,field2=value2"
            let pairs: Vec<&str> = input.split(',').collect();

            let mut fields = Vec::new();
            let mut values = Vec::new();

            for pair in pairs {
                let parts: Vec<&str> = pair.split('=').collect();
                if parts.len() != 2 {
                    return Err(SchemaError::InvalidInput("Invalid format".to_string()));
                }

                fields.push(FieldDef::new(parts[0], FieldType::String));
                values.push(SchemaValue::String(parts[1].to_string()));
            }

            let header = SchemaHeader::new(1, fields);
            IntermediateRepresentation::new(header, values)
        }
    }

    // Test the custom parser
    let input = "name=alice,age=30";
    let ir = SimpleParser::parse(input).expect("Failed to parse");

    assert_eq!(ir.header.row_count, 1);
    assert_eq!(ir.header.fields.len(), 2);
    assert_eq!(ir.header.fields[0].name, "name");
    assert_eq!(ir.header.fields[1].name, "age");

    // Verify we can pack and frame it
    let binary = pack(&ir);
    let encoded = encode_framed(&binary);
    assert!(encoded.starts_with('𓍹'));
}

#[test]
fn test_custom_serializer_implementation() {
    use base_d::schema::{
        FieldDef, FieldType, IntermediateRepresentation, OutputSerializer, SchemaError,
        SchemaHeader, SchemaValue,
    };

    // Define a minimal custom serializer (key=value format)
    struct SimpleSerializer;

    impl OutputSerializer for SimpleSerializer {
        type Error = SchemaError;

        fn serialize(
            ir: &IntermediateRepresentation,
            _pretty: bool,
        ) -> Result<String, Self::Error> {
            if ir.header.row_count != 1 {
                return Err(SchemaError::InvalidInput(
                    "Only single row supported".to_string(),
                ));
            }

            let mut parts = Vec::new();
            for (idx, field) in ir.header.fields.iter().enumerate() {
                let value = ir
                    .get_value(0, idx)
                    .ok_or_else(|| SchemaError::InvalidInput("Missing value".to_string()))?;

                let value_str = match value {
                    SchemaValue::String(s) => s.clone(),
                    SchemaValue::U64(n) => n.to_string(),
                    SchemaValue::I64(n) => n.to_string(),
                    SchemaValue::F64(n) => n.to_string(),
                    SchemaValue::Bool(b) => b.to_string(),
                    SchemaValue::Null => "null".to_string(),
                    _ => return Err(SchemaError::InvalidInput("Unsupported type".to_string())),
                };

                parts.push(format!("{}={}", field.name, value_str));
            }

            Ok(parts.join(","))
        }
    }

    // Create IR
    let fields = vec![
        FieldDef::new("name", FieldType::String),
        FieldDef::new("age", FieldType::U64),
    ];
    let header = SchemaHeader::new(1, fields);
    let values = vec![
        SchemaValue::String("alice".to_string()),
        SchemaValue::U64(30),
    ];
    let ir = IntermediateRepresentation::new(header, values).expect("Failed to create IR");

    // Serialize with custom serializer
    let output = SimpleSerializer::serialize(&ir, false).expect("Failed to serialize");
    assert_eq!(output, "name=alice,age=30");
}

#[test]
fn test_json_reference_implementations() {
    use base_d::schema::{InputParser, JsonParser, JsonSerializer, OutputSerializer};

    // Test JsonParser
    let json_input = r#"{"id":1,"name":"alice"}"#;
    let ir = JsonParser::parse(json_input).expect("Failed to parse JSON");

    assert_eq!(ir.header.row_count, 1);
    assert_eq!(ir.header.fields.len(), 2);

    // Test JsonSerializer
    let json_output = JsonSerializer::serialize(&ir, false).expect("Failed to serialize JSON");

    // Verify roundtrip
    let parsed: serde_json::Value =
        serde_json::from_str(&json_output).expect("Invalid JSON output");
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["name"], "alice");
}

#[test]
fn test_high_level_api_still_works() {
    use base_d::{SchemaCompressionAlgo, decode_schema, encode_schema};

    // Verify backward compatibility - high-level functions still work
    let json = r#"{"id":1,"name":"test"}"#;

    // Without compression
    let encoded = encode_schema(json, None).expect("Failed to encode");
    let decoded = decode_schema(&encoded, false).expect("Failed to decode");

    let original: serde_json::Value = serde_json::from_str(json).expect("Invalid JSON");
    let result: serde_json::Value = serde_json::from_str(&decoded).expect("Invalid JSON");
    assert_eq!(original, result);

    // With compression
    let compressed = encode_schema(json, Some(SchemaCompressionAlgo::Lz4))
        .expect("Failed to encode with compression");
    let decoded_compressed =
        decode_schema(&compressed, false).expect("Failed to decode compressed");

    let result_compressed: serde_json::Value =
        serde_json::from_str(&decoded_compressed).expect("Invalid JSON");
    assert_eq!(original, result_compressed);
}

#[test]
fn test_error_types_accessible() {
    use base_d::schema::SchemaError;

    // Verify error type is accessible and can be pattern matched
    let error = SchemaError::InvalidInput("test".to_string());

    match error {
        SchemaError::InvalidInput(msg) => {
            assert_eq!(msg, "test");
        }
        _ => panic!("Wrong error variant"),
    }
}

#[test]
fn test_compression_options() {
    use base_d::schema::SchemaCompressionAlgo;

    // Verify compression algorithms are accessible
    let _brotli = SchemaCompressionAlgo::Brotli;
    let _lz4 = SchemaCompressionAlgo::Lz4;
    let _zstd = SchemaCompressionAlgo::Zstd;
}

#[test]
fn test_field_type_methods() {
    use base_d::schema::FieldType;

    // Verify type tag methods work
    assert_eq!(FieldType::U64.type_tag(), 0);
    assert_eq!(FieldType::String.type_tag(), 3);

    // Verify display_name works
    assert_eq!(FieldType::U64.display_name(), "unsigned integer");
    assert_eq!(FieldType::String.display_name(), "string");

    // Verify from_type_tag works
    let reconstructed = FieldType::from_type_tag(0, None).expect("Failed to reconstruct");
    assert_eq!(reconstructed, FieldType::U64);
}

#[test]
fn test_schema_header_methods() {
    use base_d::schema::{FieldDef, FieldType, SchemaHeader};

    let fields = vec![FieldDef::new("a", FieldType::U64)];
    let mut header = SchemaHeader::new(10, fields);

    // Test flag methods
    assert!(!header.has_flag(0b0000_0001));
    header.set_flag(0b0000_0001);
    assert!(header.has_flag(0b0000_0001));

    // Test total_value_count
    assert_eq!(header.total_value_count(), 10);
}

#[test]
fn test_ir_get_value() {
    use base_d::schema::{
        FieldDef, FieldType, IntermediateRepresentation, SchemaHeader, SchemaValue,
    };

    let fields = vec![
        FieldDef::new("a", FieldType::U64),
        FieldDef::new("b", FieldType::String),
    ];
    let header = SchemaHeader::new(2, fields);
    let values = vec![
        SchemaValue::U64(1),
        SchemaValue::String("x".to_string()),
        SchemaValue::U64(2),
        SchemaValue::String("y".to_string()),
    ];
    let ir = IntermediateRepresentation::new(header, values).expect("Failed to create IR");

    // Test get_value
    assert_eq!(ir.get_value(0, 0), Some(&SchemaValue::U64(1)));
    assert_eq!(
        ir.get_value(0, 1),
        Some(&SchemaValue::String("x".to_string()))
    );
    assert_eq!(ir.get_value(1, 0), Some(&SchemaValue::U64(2)));
    assert_eq!(
        ir.get_value(1, 1),
        Some(&SchemaValue::String("y".to_string()))
    );
    assert_eq!(ir.get_value(2, 0), None); // Out of bounds
}
