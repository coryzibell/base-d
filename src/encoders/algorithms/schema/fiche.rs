//! fiche format: model-readable structured data
//!
//! fiche is the model-readable sibling to carrier98. While carrier98 is opaque
//! (maximum density, model shuttles without parsing), fiche uses Unicode delimiters
//! so models can parse structure with minimal tokens.
//!
//! # Delimiters
//!
//! | Symbol | Unicode | Purpose |
//! |--------|---------|---------|
//! | `◉` | U+25C9 | Row start (fisheye) |
//! | `┃` | U+2503 | Field separator (heavy pipe) |
//! | `◈` | U+25C8 | Array element separator |
//! | `∅` | U+2205 | Null value |
//!
//! # Format
//!
//! ```text
//! @{root}┃{field}:{type}┃{field}:{type}...
//! ◉{value}┃{value}┃{value}...
//! ◉{value}┃{value}┃{value}...
//! ```
//!
//! # Example
//!
//! ```text
//! @users┃id:int┃name:str┃active:bool
//! ◉1┃alice┃true
//! ◉2┃bob┃false
//! ```

use super::types::{
    FLAG_HAS_NULLS, FLAG_HAS_ROOT_KEY, FieldDef, FieldType, IntermediateRepresentation,
    SchemaError, SchemaHeader, SchemaValue,
};

// Fiche delimiters
pub const ROW_START: char = '◉'; // U+25C9 fisheye
pub const FIELD_SEP: char = '┃'; // U+2503 heavy pipe
pub const ARRAY_SEP: char = '◈'; // U+25C8 diamond in diamond
pub const NULL_VALUE: &str = "∅"; // U+2205 empty set

// Type names in fiche schema
pub const TYPE_INT: &str = "int";
pub const TYPE_STR: &str = "str";
pub const TYPE_FLOAT: &str = "float";
pub const TYPE_BOOL: &str = "bool";

/// Serialize IR to fiche format
pub fn serialize(ir: &IntermediateRepresentation) -> Result<String, SchemaError> {
    let mut output = String::new();

    // Schema line: @{root}┃{field}:{type}...
    output.push('@');
    if let Some(ref root_key) = ir.header.root_key {
        output.push_str(root_key);
    }

    for field in &ir.header.fields {
        output.push(FIELD_SEP);
        output.push_str(&field.name);
        output.push(':');
        output.push_str(&field_type_to_str(&field.field_type));
    }
    output.push('\n');

    // Data rows: ◉{value}┃{value}...
    let field_count = ir.header.fields.len();
    for row in 0..ir.header.row_count {
        output.push(ROW_START);

        for (field_idx, field) in ir.header.fields.iter().enumerate() {
            if field_idx > 0 {
                output.push(FIELD_SEP);
            }

            // Check null bitmap
            if ir.is_null(row, field_idx) {
                output.push_str(NULL_VALUE);
            } else {
                let value_idx = row * field_count + field_idx;
                let value = &ir.values[value_idx];
                output.push_str(&value_to_str(value, &field.field_type));
            }
        }
        output.push('\n');
    }

    Ok(output.trim_end().to_string())
}

/// Parse fiche format to IR
pub fn parse(input: &str) -> Result<IntermediateRepresentation, SchemaError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(SchemaError::InvalidInput("Empty fiche input".to_string()));
    }

    // Split into schema line and data
    let row_marker = ROW_START.to_string();
    let first_row_pos = input.find(&row_marker);

    let (schema_part, data_part) = if let Some(pos) = first_row_pos {
        (&input[..pos], &input[pos..])
    } else {
        return Err(SchemaError::InvalidInput(
            "No data rows found (missing ◉ row marker)".to_string(),
        ));
    };

    // Parse schema line
    let schema_line = schema_part.trim();
    if !schema_line.starts_with('@') {
        return Err(SchemaError::InvalidInput(
            "Schema line must start with @".to_string(),
        ));
    }

    let schema_content = &schema_line[1..]; // Remove @
    let schema_parts: Vec<&str> = schema_content.split(FIELD_SEP).collect();

    // First part is root key (may be empty)
    let root_key = if schema_parts.is_empty() || schema_parts[0].is_empty() {
        None
    } else if schema_parts[0].contains(':') {
        // No root key, first part is a field
        None
    } else {
        Some(schema_parts[0].to_string())
    };

    // Parse field definitions
    let field_start = if root_key.is_some() { 1 } else { 0 };
    let mut fields = Vec::new();

    for part in schema_parts.iter().skip(field_start) {
        if part.is_empty() {
            continue;
        }
        let (name, field_type) = parse_field_def(part)?;
        fields.push(FieldDef::new(name, field_type));
    }

    if fields.is_empty() {
        return Err(SchemaError::InvalidInput(
            "No field definitions in schema".to_string(),
        ));
    }

    // Parse data rows
    let mut values = Vec::new();
    let mut null_positions = Vec::new();
    let mut row_count = 0;

    // Split by row marker
    for row_str in data_part.split(ROW_START) {
        let row_str = row_str.trim();
        if row_str.is_empty() {
            continue;
        }

        // Handle multiline content - find next row or end
        let row_values: Vec<&str> = split_row(row_str, &fields);

        if row_values.len() != fields.len() {
            return Err(SchemaError::InvalidInput(format!(
                "Row {} has {} values, expected {} fields",
                row_count,
                row_values.len(),
                fields.len()
            )));
        }

        for (field_idx, (value_str, field)) in row_values.iter().zip(fields.iter()).enumerate() {
            let value_str = value_str.trim();

            if value_str == NULL_VALUE {
                null_positions.push(row_count * fields.len() + field_idx);
                values.push(SchemaValue::Null);
            } else {
                let value = parse_value(value_str, &field.field_type)?;
                values.push(value);
            }
        }

        row_count += 1;
    }

    // Build header
    let mut header = SchemaHeader::new(row_count, fields);
    if root_key.is_some() {
        header.root_key = root_key;
        header.set_flag(FLAG_HAS_ROOT_KEY);
    }

    // Build null bitmap if we have nulls
    if !null_positions.is_empty() {
        header.set_flag(FLAG_HAS_NULLS);
        let bitmap_size = (row_count * header.fields.len()).div_ceil(8);
        let mut bitmap = vec![0u8; bitmap_size];

        for pos in null_positions {
            let byte_idx = pos / 8;
            let bit_idx = pos % 8;
            bitmap[byte_idx] |= 1 << bit_idx;
        }
        header.null_bitmap = Some(bitmap);
    }

    IntermediateRepresentation::new(header, values)
}

/// Convert FieldType to fiche type string
fn field_type_to_str(ft: &FieldType) -> String {
    match ft {
        FieldType::U64 | FieldType::I64 => TYPE_INT.to_string(),
        FieldType::F64 => TYPE_FLOAT.to_string(),
        FieldType::String => TYPE_STR.to_string(),
        FieldType::Bool => TYPE_BOOL.to_string(),
        FieldType::Null => TYPE_STR.to_string(), // Nulls rendered as str type
        FieldType::Array(inner) => format!("{}[]", field_type_to_str(inner)),
        FieldType::Any => TYPE_STR.to_string(),
    }
}

/// Parse fiche type string to FieldType
fn parse_type_str(s: &str) -> Result<FieldType, SchemaError> {
    if let Some(inner) = s.strip_suffix("[]") {
        let inner_type = parse_type_str(inner)?;
        return Ok(FieldType::Array(Box::new(inner_type)));
    }

    match s {
        TYPE_INT => Ok(FieldType::I64), // Default to signed for flexibility
        TYPE_STR => Ok(FieldType::String),
        TYPE_FLOAT => Ok(FieldType::F64),
        TYPE_BOOL => Ok(FieldType::Bool),
        _ => Err(SchemaError::InvalidInput(format!(
            "Unknown type '{}'. Valid types: int, str, float, bool",
            s
        ))),
    }
}

/// Parse field definition like "name:str" or "tags:str[]"
fn parse_field_def(s: &str) -> Result<(String, FieldType), SchemaError> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(SchemaError::InvalidInput(format!(
            "Invalid field definition '{}'. Expected format: name:type",
            s
        )));
    }

    let name = parts[0].trim().to_string();
    let field_type = parse_type_str(parts[1].trim())?;

    Ok((name, field_type))
}

/// Convert SchemaValue to fiche string
fn value_to_str(value: &SchemaValue, field_type: &FieldType) -> String {
    match value {
        SchemaValue::U64(n) => n.to_string(),
        SchemaValue::I64(n) => n.to_string(),
        SchemaValue::F64(n) => {
            // Preserve integer-like floats without decimal
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{:.1}", n)
            } else {
                n.to_string()
            }
        }
        SchemaValue::String(s) => s.clone(),
        SchemaValue::Bool(b) => b.to_string(),
        SchemaValue::Null => NULL_VALUE.to_string(),
        SchemaValue::Array(arr) => {
            let inner_type = if let FieldType::Array(inner) = field_type {
                inner.as_ref()
            } else {
                &FieldType::String
            };
            arr.iter()
                .map(|v| value_to_str(v, inner_type))
                .collect::<Vec<_>>()
                .join(&ARRAY_SEP.to_string())
        }
    }
}

/// Parse value string to SchemaValue
fn parse_value(s: &str, field_type: &FieldType) -> Result<SchemaValue, SchemaError> {
    match field_type {
        FieldType::U64 => s
            .parse::<u64>()
            .map(SchemaValue::U64)
            .map_err(|_| SchemaError::InvalidInput(format!("Invalid integer: '{}'", s))),
        FieldType::I64 => s
            .parse::<i64>()
            .map(SchemaValue::I64)
            .map_err(|_| SchemaError::InvalidInput(format!("Invalid integer: '{}'", s))),
        FieldType::F64 => s
            .parse::<f64>()
            .map(SchemaValue::F64)
            .map_err(|_| SchemaError::InvalidInput(format!("Invalid float: '{}'", s))),
        FieldType::String => Ok(SchemaValue::String(s.to_string())),
        FieldType::Bool => match s {
            "true" => Ok(SchemaValue::Bool(true)),
            "false" => Ok(SchemaValue::Bool(false)),
            _ => Err(SchemaError::InvalidInput(format!(
                "Invalid boolean: '{}'. Expected 'true' or 'false'",
                s
            ))),
        },
        FieldType::Null => Ok(SchemaValue::Null),
        FieldType::Array(inner) => {
            if s.is_empty() {
                return Ok(SchemaValue::Array(vec![]));
            }
            let elements: Result<Vec<_>, _> = s
                .split(ARRAY_SEP)
                .map(|elem| parse_value(elem.trim(), inner))
                .collect();
            elements.map(SchemaValue::Array)
        }
        FieldType::Any => Ok(SchemaValue::String(s.to_string())),
    }
}

/// Split a row string by field separator, handling the known field count
fn split_row<'a>(row_str: &'a str, fields: &[FieldDef]) -> Vec<&'a str> {
    let sep = FIELD_SEP.to_string();
    let parts: Vec<&str> = row_str.splitn(fields.len(), &sep).collect();
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_roundtrip() {
        let fiche = "@users┃id:int┃name:str┃active:bool
◉1┃alice┃true
◉2┃bob┃false";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.row_count, 2);
        assert_eq!(ir.header.fields.len(), 3);
        assert_eq!(ir.header.root_key, Some("users".to_string()));

        let output = serialize(&ir).unwrap();
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_arrays() {
        let fiche = "@users┃id:int┃tags:str[]
◉1┃admin◈editor
◉2┃viewer";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.row_count, 2);

        // Check first row's tags
        if let Some(SchemaValue::Array(tags)) = ir.get_value(0, 1) {
            assert_eq!(tags.len(), 2);
        } else {
            panic!("Expected array");
        }

        let output = serialize(&ir).unwrap();
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_nulls() {
        let fiche = "@records┃id:int┃score:float┃notes:str
◉1┃95.5┃∅
◉2┃∅┃pending";

        let ir = parse(fiche).unwrap();
        assert!(ir.is_null(0, 2)); // notes is null for row 0
        assert!(ir.is_null(1, 1)); // score is null for row 1

        let output = serialize(&ir).unwrap();
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_embedded_json() {
        let fiche = r#"@logs┃level:str┃msg:str
◉error┃Failed to parse {"key": "value"}"#;

        let ir = parse(fiche).unwrap();

        if let Some(SchemaValue::String(msg)) = ir.get_value(0, 1) {
            assert_eq!(msg, r#"Failed to parse {"key": "value"}"#);
        } else {
            panic!("Expected string");
        }

        let output = serialize(&ir).unwrap();
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_no_root_key() {
        let fiche = "@┃id:int┃name:str
◉1┃alice";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.root_key, None);
    }

    #[test]
    fn test_type_parsing() {
        assert!(matches!(parse_type_str("int"), Ok(FieldType::I64)));
        assert!(matches!(parse_type_str("str"), Ok(FieldType::String)));
        assert!(matches!(parse_type_str("float"), Ok(FieldType::F64)));
        assert!(matches!(parse_type_str("bool"), Ok(FieldType::Bool)));
        assert!(matches!(
            parse_type_str("str[]"),
            Ok(FieldType::Array(box_inner)) if *box_inner == FieldType::String
        ));
    }
}
