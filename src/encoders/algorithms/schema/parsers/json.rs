use crate::encoders::algorithms::schema::fiche::NEST_SEP;
use crate::encoders::algorithms::schema::parsers::InputParser;
use crate::encoders::algorithms::schema::types::*;
use serde_json::{Map, Value};
use std::collections::HashMap;

pub struct JsonParser;

impl InputParser for JsonParser {
    type Error = SchemaError;

    fn parse(input: &str) -> Result<IntermediateRepresentation, Self::Error> {
        let parsed: Value = serde_json::from_str(input).map_err(|e| {
            SchemaError::InvalidInput(format!(
                "Invalid JSON syntax: {}\n\
                 Ensure the input is valid JSON.",
                e
            ))
        })?;

        match parsed {
            Value::Array(arr) => parse_array(arr),
            Value::Object(obj) => parse_object(obj),
            _ => Err(SchemaError::InvalidInput(
                "Expected JSON object or array at root level.\n\
                 Schema encoding works with:\n\
                 - Single object: {\"name\": \"value\"}\n\
                 - Array of objects: [{\"id\": 1}, {\"id\": 2}]\n\
                 - Object with array: {\"users\": [{\"id\": 1}]}"
                    .to_string(),
            )),
        }
    }
}

/// Parse array of objects (tabular data)
fn parse_array(arr: Vec<Value>) -> Result<IntermediateRepresentation, SchemaError> {
    if arr.is_empty() {
        return Err(SchemaError::InvalidInput(
            "Empty array - cannot infer schema from zero rows.\n\
             Provide at least one object in the array."
                .to_string(),
        ));
    }

    let row_count = arr.len();
    let mut all_rows: Vec<Map<String, Value>> = Vec::new();

    // Extract objects from array
    for (idx, item) in arr.into_iter().enumerate() {
        match item {
            Value::Object(obj) => all_rows.push(obj),
            other => {
                let type_name = match other {
                    Value::Null => "null",
                    Value::Bool(_) => "boolean",
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    Value::Array(_) => "array",
                    Value::Object(_) => unreachable!(),
                };
                return Err(SchemaError::InvalidInput(format!(
                    "Array must contain only objects (tabular data). Found {} at index {}.\n\
                     Schema encoding expects arrays of objects like: [{{\"id\": 1}}, {{\"id\": 2}}]",
                    type_name, idx
                )));
            }
        }
    }

    // Flatten all objects and collect field names
    let mut flattened_rows: Vec<HashMap<String, Value>> = Vec::new();
    let mut all_field_names = std::collections::BTreeSet::new();
    let mut array_markers = std::collections::BTreeSet::new();

    for obj in &all_rows {
        let flattened = flatten_object(obj, "");
        for key in flattened.keys() {
            if key.ends_with("[]") {
                // This is an array marker, track it separately
                array_markers.insert(key.clone());
            } else {
                all_field_names.insert(key.clone());
            }
        }
        flattened_rows.push(flattened);
    }

    // Add array markers as fields with special marker type
    let mut field_names: Vec<String> = all_field_names.into_iter().collect();
    let array_marker_names: Vec<String> = array_markers.into_iter().collect();
    field_names.extend(array_marker_names);

    // Infer types and build fields
    let mut fields = Vec::new();
    let mut has_nulls = false;

    for field_name in &field_names {
        if field_name.ends_with("[]") {
            // Array marker - use a special type to indicate this is metadata
            fields.push(FieldDef::new(field_name.clone(), FieldType::Null));
        } else {
            let field_type = infer_field_type(&flattened_rows, field_name, &mut has_nulls)?;
            fields.push(FieldDef::new(field_name.clone(), field_type));
        }
    }

    // Build values and null bitmap
    let mut values = Vec::new();
    let total_values = row_count * fields.len();
    let bitmap_bytes = total_values.div_ceil(8);
    let mut null_bitmap = vec![0u8; bitmap_bytes];

    for (row_idx, row) in flattened_rows.iter().enumerate() {
        for (field_idx, field) in fields.iter().enumerate() {
            let value_idx = row_idx * fields.len() + field_idx;

            // Handle array markers - always null
            if field.name.ends_with("[]") {
                values.push(SchemaValue::Null);
                set_null_bit(&mut null_bitmap, value_idx);
                has_nulls = true;
                continue;
            }

            if let Some(json_value) = row.get(&field.name)
                && json_value.is_null()
            {
                values.push(SchemaValue::Null);
                set_null_bit(&mut null_bitmap, value_idx);
                has_nulls = true;
            } else if let Some(json_value) = row.get(&field.name) {
                values.push(json_to_schema_value(json_value, &field.field_type)?);
            } else {
                // Missing field = null
                values.push(SchemaValue::Null);
                set_null_bit(&mut null_bitmap, value_idx);
                has_nulls = true;
            }
        }
    }

    // Build header
    let mut header = SchemaHeader::new(row_count, fields);
    if has_nulls {
        header.null_bitmap = Some(null_bitmap);
        header.set_flag(FLAG_HAS_NULLS);
    }

    IntermediateRepresentation::new(header, values)
}

/// Parse single object (may have root key)
fn parse_object(obj: Map<String, Value>) -> Result<IntermediateRepresentation, SchemaError> {
    // Check for common pagination wrapper keys
    const WRAPPER_KEYS: &[&str] = &["results", "data", "items", "records"];

    // Check for metadata pattern: scalar fields + one array field
    let mut array_field: Option<(String, Vec<Value>)> = None;
    let mut scalar_fields: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for (key, value) in &obj {
        match value {
            Value::Array(arr)
                if !arr.is_empty() && arr.iter().all(|item| matches!(item, Value::Object(_))) =>
            {
                if array_field.is_none() {
                    array_field = Some((key.clone(), arr.clone()));
                } else {
                    // Multiple arrays - not metadata pattern
                    array_field = None;
                    scalar_fields.clear();
                    break;
                }
            }
            Value::String(s) => {
                scalar_fields.insert(key.clone(), s.clone());
            }
            Value::Number(n) => {
                scalar_fields.insert(key.clone(), n.to_string());
            }
            Value::Bool(b) => {
                scalar_fields.insert(key.clone(), b.to_string());
            }
            Value::Null => {
                // Encode null metadata as ∅ symbol
                scalar_fields.insert(key.clone(), "∅".to_string());
            }
            _ => {
                // Non-scalar or nested object - not metadata pattern
                scalar_fields.clear();
                array_field = None;
                break;
            }
        }
    }

    // If we have exactly one array field and at least one scalar field, extract metadata
    if let Some((array_key, arr)) = array_field
        && !scalar_fields.is_empty()
    {
        let mut ir = parse_array(arr)?;
        ir.header.root_key = Some(array_key);
        ir.header.set_flag(FLAG_HAS_ROOT_KEY);
        ir.header.metadata = Some(scalar_fields);
        return Ok(ir);
    }

    // Check if this is a wrapper object with one of the known keys
    if obj.len() == 1 {
        // Check if value is an array of objects before consuming
        let is_root_key_pattern = obj
            .values()
            .next()
            .map(|v| {
                if let Value::Array(arr) = v {
                    // Only treat as root key if array contains objects (tabular data)
                    !arr.is_empty() && arr.iter().all(|item| matches!(item, Value::Object(_)))
                } else {
                    false
                }
            })
            .unwrap_or(false);

        if is_root_key_pattern {
            // Extract key and value by consuming the map
            let (key, value) = obj.into_iter().next().unwrap();
            // We already checked it's an array
            let arr = match value {
                Value::Array(a) => a,
                _ => unreachable!(),
            };

            // Parse as array with root key
            let mut ir = parse_array(arr)?;
            ir.header.root_key = Some(key);
            ir.header.set_flag(FLAG_HAS_ROOT_KEY);
            return Ok(ir);
        }
    }

    // Check for known wrapper patterns and unwrap them
    for wrapper_key in WRAPPER_KEYS {
        if let Some(Value::Array(arr)) = obj.get(*wrapper_key)
            && !arr.is_empty()
            && arr.iter().all(|item| matches!(item, Value::Object(_)))
        {
            // Found a wrapper key - unwrap and parse the array
            let arr = arr.clone();
            let mut ir = parse_array(arr)?;
            ir.header.root_key = Some((*wrapper_key).to_string());
            ir.header.set_flag(FLAG_HAS_ROOT_KEY);
            return Ok(ir);
        }
    }

    // Single object - treat as single row
    let flattened = flatten_object(&obj, "");
    // Preserve field order from original object (serde_json preserves insertion order)
    let mut field_names = Vec::new();
    let mut array_markers = Vec::new();
    collect_field_names_ordered(&obj, "", &mut field_names);

    // Separate array markers from regular fields
    let mut regular_fields = Vec::new();
    for name in field_names {
        if name.ends_with("[]") {
            array_markers.push(name);
        } else {
            regular_fields.push(name);
        }
    }
    // Add array markers at the end
    regular_fields.extend(array_markers);
    let field_names = regular_fields;

    let mut fields = Vec::new();
    let mut has_nulls = false;

    for field_name in &field_names {
        if field_name.ends_with("[]") {
            // Array marker
            fields.push(FieldDef::new(field_name.clone(), FieldType::Null));
            has_nulls = true;
        } else {
            let value = &flattened[field_name];
            let field_type = infer_type(value);
            if value.is_null() {
                has_nulls = true;
            }
            fields.push(FieldDef::new(field_name.clone(), field_type));
        }
    }

    // Build values and null bitmap
    let mut values = Vec::new();
    let total_values = fields.len();
    let bitmap_bytes = total_values.div_ceil(8);
    let mut null_bitmap = vec![0u8; bitmap_bytes];

    for (field_idx, field) in fields.iter().enumerate() {
        // Handle array markers
        if field.name.ends_with("[]") {
            values.push(SchemaValue::Null);
            set_null_bit(&mut null_bitmap, field_idx);
            continue;
        }

        let json_value = &flattened[&field.name];
        if json_value.is_null() {
            values.push(SchemaValue::Null);
            set_null_bit(&mut null_bitmap, field_idx);
        } else {
            values.push(json_to_schema_value(json_value, &field.field_type)?);
        }
    }

    // Build header
    let mut header = SchemaHeader::new(1, fields);
    if has_nulls {
        header.null_bitmap = Some(null_bitmap);
        header.set_flag(FLAG_HAS_NULLS);
    }

    IntermediateRepresentation::new(header, values)
}

/// Collect field names in order from nested object
fn collect_field_names_ordered(obj: &Map<String, Value>, prefix: &str, names: &mut Vec<String>) {
    for (key, value) in obj {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}{}{}", prefix, NEST_SEP, key)
        };

        match value {
            Value::Object(nested) => {
                collect_field_names_ordered(nested, &full_key, names);
            }
            Value::Array(arr) => {
                // Mark this as an array
                names.push(format!("{}[]", full_key));

                // Collect indexed field names for array elements
                for (idx, item) in arr.iter().enumerate() {
                    let indexed_key = format!("{}{}{}", full_key, NEST_SEP, idx);
                    collect_field_names_from_value(item, &indexed_key, names);
                }
            }
            _ => {
                names.push(full_key);
            }
        }
    }
}

/// Helper to collect field names from any value type
fn collect_field_names_from_value(value: &Value, prefix: &str, names: &mut Vec<String>) {
    match value {
        Value::Object(obj) => {
            collect_field_names_ordered(obj, prefix, names);
        }
        Value::Array(arr) => {
            // Mark this as an array
            names.push(format!("{}[]", prefix));

            for (idx, item) in arr.iter().enumerate() {
                let indexed_key = format!("{}{}{}", prefix, NEST_SEP, idx);
                collect_field_names_from_value(item, &indexed_key, names);
            }
        }
        _ => {
            names.push(prefix.to_string());
        }
    }
}

/// Flatten nested object with NEST_SEP delimiter
/// Returns (flattened_map, array_paths) where array_paths tracks which keys are arrays
fn flatten_object(obj: &Map<String, Value>, prefix: &str) -> HashMap<String, Value> {
    let mut result = HashMap::new();

    for (key, value) in obj {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}{}{}", prefix, NEST_SEP, key)
        };

        match value {
            Value::Object(nested) => {
                result.extend(flatten_object(nested, &full_key));
            }
            Value::Array(arr) => {
                // Mark this key as an array by inserting a marker
                result.insert(format!("{}[]", full_key), Value::Null);

                // Flatten array elements with indexed keys
                for (idx, item) in arr.iter().enumerate() {
                    let indexed_key = format!("{}{}{}", full_key, NEST_SEP, idx);
                    match item {
                        Value::Object(nested_obj) => {
                            // Recursively flatten nested object
                            result.extend(flatten_object(nested_obj, &indexed_key));
                        }
                        Value::Array(nested_arr) => {
                            // Recursively handle nested arrays
                            for (nested_idx, nested_item) in nested_arr.iter().enumerate() {
                                let nested_indexed_key = format!("{}{}{}", indexed_key, NEST_SEP, nested_idx);
                                flatten_value(&nested_indexed_key, nested_item, &mut result);
                            }
                        }
                        _ => {
                            // Primitive values get direct insertion
                            result.insert(indexed_key, item.clone());
                        }
                    }
                }
            }
            _ => {
                result.insert(full_key, value.clone());
            }
        }
    }

    result
}

/// Helper function to recursively flatten any value type
fn flatten_value(key: &str, value: &Value, result: &mut HashMap<String, Value>) {
    match value {
        Value::Object(obj) => {
            result.extend(flatten_object(obj, key));
        }
        Value::Array(arr) => {
            // Mark this key as an array
            result.insert(format!("{}[]", key), Value::Null);

            for (idx, item) in arr.iter().enumerate() {
                let indexed_key = format!("{}{}{}", key, NEST_SEP, idx);
                flatten_value(&indexed_key, item, result);
            }
        }
        _ => {
            result.insert(key.to_string(), value.clone());
        }
    }
}

/// Infer type from a single JSON value
fn infer_type(value: &Value) -> FieldType {
    match value {
        Value::Null => FieldType::Null,
        Value::Bool(_) => FieldType::Bool,
        Value::Number(n) => {
            if n.is_f64() {
                // Check if it has a fractional part
                if let Some(f) = n.as_f64()
                    && (f.fract() != 0.0 || f.is_infinite() || f.is_nan())
                {
                    return FieldType::F64;
                }
            }

            if let Some(i) = n.as_i64() {
                if i < 0 {
                    FieldType::I64
                } else {
                    FieldType::U64
                }
            } else if n.as_u64().is_some() {
                FieldType::U64
            } else {
                FieldType::F64
            }
        }
        Value::String(_) => FieldType::String,
        Value::Array(arr) => {
            if arr.is_empty() {
                FieldType::Array(Box::new(FieldType::Null))
            } else {
                // Infer from first non-null element
                let element_type = arr
                    .iter()
                    .find(|v| !v.is_null())
                    .map(infer_type)
                    .unwrap_or(FieldType::Null);
                FieldType::Array(Box::new(element_type))
            }
        }
        Value::Object(_) => {
            // This shouldn't happen after flattening
            FieldType::String
        }
    }
}

/// Infer field type across multiple rows
fn infer_field_type(
    rows: &[HashMap<String, Value>],
    field_name: &str,
    has_nulls: &mut bool,
) -> Result<FieldType, SchemaError> {
    let mut inferred_type: Option<FieldType> = None;

    for row in rows {
        if let Some(value) = row.get(field_name) {
            if value.is_null() {
                *has_nulls = true;
                continue;
            }

            let current_type = infer_type(value);

            if let Some(ref existing_type) = inferred_type {
                // Special case: Array(Null) unifies with Array(T) → Array(T)
                if let (FieldType::Array(existing_inner), FieldType::Array(current_inner)) =
                    (existing_type, &current_type)
                {
                    if **existing_inner == FieldType::Null && **current_inner != FieldType::Null {
                        // Upgrade from Array(Null) to Array(T)
                        inferred_type = Some(current_type.clone());
                        continue;
                    } else if **current_inner == FieldType::Null
                        && **existing_inner != FieldType::Null
                    {
                        // Keep existing Array(T), ignore Array(Null)
                        continue;
                    }
                }

                if *existing_type != current_type {
                    // Type conflict - use Any
                    return Ok(FieldType::Any);
                }
            } else {
                inferred_type = Some(current_type);
            }
        } else {
            *has_nulls = true;
        }
    }

    Ok(inferred_type.unwrap_or(FieldType::Null))
}

/// Convert JSON value to SchemaValue
fn json_to_schema_value(
    value: &Value,
    expected_type: &FieldType,
) -> Result<SchemaValue, SchemaError> {
    match value {
        Value::Null => Ok(SchemaValue::Null),
        Value::Bool(b) => Ok(SchemaValue::Bool(*b)),
        Value::Number(n) => match expected_type {
            FieldType::U64 | FieldType::Any => {
                if let Some(u) = n.as_u64() {
                    Ok(SchemaValue::U64(u))
                } else if let Some(i) = n.as_i64() {
                    Ok(SchemaValue::I64(i))
                } else {
                    Ok(SchemaValue::F64(n.as_f64().unwrap()))
                }
            }
            FieldType::I64 => {
                if let Some(i) = n.as_i64() {
                    Ok(SchemaValue::I64(i))
                } else {
                    Ok(SchemaValue::I64(n.as_f64().unwrap() as i64))
                }
            }
            FieldType::F64 => Ok(SchemaValue::F64(n.as_f64().unwrap())),
            _ => Err(SchemaError::InvalidInput(format!(
                "Type mismatch: expected {}, but found number.\n\
                 The field type was inferred or specified as {}, which doesn't accept numeric values.",
                expected_type.display_name(),
                expected_type.display_name()
            ))),
        },
        Value::String(s) => Ok(SchemaValue::String(s.clone())),
        Value::Array(arr) => {
            let element_type = if let FieldType::Array(et) = expected_type {
                et.as_ref()
            } else {
                return Err(SchemaError::InvalidInput(format!(
                    "Internal error: Expected array type but found {}. This is a bug in type inference.",
                    expected_type.display_name()
                )));
            };

            let mut schema_values = Vec::new();
            for item in arr {
                schema_values.push(json_to_schema_value(item, element_type)?);
            }
            Ok(SchemaValue::Array(schema_values))
        }
        Value::Object(_) => Err(SchemaError::InvalidInput(
            "Internal error: Encountered nested object that wasn't flattened. This is a bug in the JSON parser."
                .to_string(),
        )),
    }
}

/// Set a bit in the null bitmap
fn set_null_bit(bitmap: &mut [u8], index: usize) {
    let byte_idx = index / 8;
    let bit_idx = index % 8;
    bitmap[byte_idx] |= 1 << bit_idx;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_object() {
        let input = r#"{"id":1,"name":"alice"}"#;
        let ir = JsonParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(ir.header.fields.len(), 2);
        assert_eq!(ir.values.len(), 2);
    }

    #[test]
    fn test_array_of_objects() {
        let input = r#"[{"id":1,"name":"alice"},{"id":2,"name":"bob"}]"#;
        let ir = JsonParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 2);
        assert_eq!(ir.header.fields.len(), 2);
        assert_eq!(ir.values.len(), 4);
    }

    #[test]
    fn test_nested_object() {
        let input = r#"{"user":{"profile":{"name":"alice"}}}"#;
        let ir = JsonParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(ir.header.fields.len(), 1);
        assert_eq!(ir.header.fields[0].name, "user჻profile჻name");
    }

    #[test]
    fn test_root_key() {
        let input = r#"{"users":[{"id":1}]}"#;
        let ir = JsonParser::parse(input).unwrap();

        assert_eq!(ir.header.root_key, Some("users".to_string()));
        assert!(ir.header.has_flag(FLAG_HAS_ROOT_KEY));
    }

    #[test]
    fn test_all_types() {
        let input = r#"{"u":1,"i":-1,"f":3.14,"s":"test","b":true,"n":null}"#;
        let ir = JsonParser::parse(input).unwrap();

        assert_eq!(ir.header.fields.len(), 6);
        assert!(ir.header.has_flag(FLAG_HAS_NULLS));
    }

    #[test]
    fn test_null_handling() {
        let input = r#"{"name":"alice","age":null}"#;
        let ir = JsonParser::parse(input).unwrap();

        assert!(ir.header.has_flag(FLAG_HAS_NULLS));

        // Find which field is "age"
        let age_idx = ir
            .header
            .fields
            .iter()
            .position(|f| f.name == "age")
            .unwrap();
        assert!(ir.is_null(0, age_idx)); // age field is null
    }

    #[test]
    fn test_homogeneous_array() {
        // Arrays now flatten to indexed fields plus array marker
        let input = r#"{"scores":[1,2,3]}"#;
        let ir = JsonParser::parse(input).unwrap();

        // Should have 4 fields: scores.0, scores.1, scores.2, scores[]
        assert_eq!(ir.header.fields.len(), 4);
        assert_eq!(ir.header.fields[0].name, "scores჻0");
        assert_eq!(ir.header.fields[0].field_type, FieldType::U64);
        assert_eq!(ir.header.fields[1].name, "scores჻1");
        assert_eq!(ir.header.fields[2].name, "scores჻2");
        assert_eq!(ir.header.fields[3].name, "scores[]");
    }

    #[test]
    fn test_empty_array() {
        // Empty arrays flatten to just the array marker
        let input = r#"{"items":[]}"#;
        let ir = JsonParser::parse(input).unwrap();

        // Empty array produces just the marker field
        assert_eq!(ir.header.fields.len(), 1);
        assert_eq!(ir.header.fields[0].name, "items[]");
    }

    #[test]
    fn test_deep_nesting() {
        let input = r#"{"a":{"b":{"c":{"d":1}}}}"#;
        let ir = JsonParser::parse(input).unwrap();

        assert_eq!(ir.header.fields[0].name, "a჻b჻c჻d");
    }

    #[test]
    fn test_flatten_object() {
        let obj: Map<String, Value> = serde_json::from_str(r#"{"a":{"b":1}}"#).unwrap();
        let flattened = flatten_object(&obj, "");

        assert_eq!(flattened.len(), 1);
        assert!(flattened.contains_key("a჻b"));
    }

    #[test]
    fn test_single_level_nesting() {
        let input = r#"{"id":"A1","name":"Jim","grade":{"math":60,"physics":66,"chemistry":61}}"#;
        let ir = JsonParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(ir.header.fields.len(), 5);

        // Check field names
        let field_names: Vec<String> = ir.header.fields.iter().map(|f| f.name.clone()).collect();
        assert!(field_names.contains(&"id".to_string()));
        assert!(field_names.contains(&"name".to_string()));
        assert!(field_names.contains(&"grade჻math".to_string()));
        assert!(field_names.contains(&"grade჻physics".to_string()));
        assert!(field_names.contains(&"grade჻chemistry".to_string()));
    }

    #[test]
    fn test_array_of_nested_objects() {
        let input = r#"{"students":[{"id":"A1","name":"Jim","grade":{"math":60,"physics":66}}]}"#;
        let ir = JsonParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(ir.header.root_key, Some("students".to_string()));

        let field_names: Vec<String> = ir.header.fields.iter().map(|f| f.name.clone()).collect();
        assert!(field_names.contains(&"id".to_string()));
        assert!(field_names.contains(&"name".to_string()));
        assert!(field_names.contains(&"grade჻math".to_string()));
        assert!(field_names.contains(&"grade჻physics".to_string()));
    }

    #[test]
    fn test_multiple_nested_levels() {
        let input = r#"{"data":{"user":{"profile":{"address":{"city":"Boston"}}}}}"#;
        let ir = JsonParser::parse(input).unwrap();

        assert_eq!(ir.header.fields.len(), 1);
        assert_eq!(ir.header.fields[0].name, "data჻user჻profile჻address჻city");
    }

    #[test]
    fn test_mixed_arrays_and_objects() {
        // Arrays now flatten to indexed fields
        let input =
            r#"{"person":{"name":"Alice","tags":["admin","user"],"address":{"city":"NYC"}}}"#;
        let ir = JsonParser::parse(input).unwrap();

        let field_names: Vec<String> = ir.header.fields.iter().map(|f| f.name.clone()).collect();
        assert!(field_names.contains(&"person჻name".to_string()));
        // tags array flattens to indexed fields
        assert!(field_names.contains(&"person჻tags჻0".to_string()));
        assert!(field_names.contains(&"person჻tags჻1".to_string()));
        assert!(field_names.contains(&"person჻address჻city".to_string()));

        // Verify tags.0 is a string type (no longer Array)
        let tags_field = ir
            .header
            .fields
            .iter()
            .find(|f| f.name == "person჻tags჻0")
            .unwrap();
        assert_eq!(tags_field.field_type, FieldType::String);
    }

    #[test]
    fn test_metadata_pattern() {
        let input = r#"{"school_name": "Springfield High", "class": "Year 1", "students": [{"id": "A1"}, {"id": "B2"}]}"#;
        let ir = JsonParser::parse(input).unwrap();

        // Should extract metadata
        assert!(ir.header.metadata.is_some());
        let metadata = ir.header.metadata.as_ref().unwrap();
        assert_eq!(
            metadata.get("school_name"),
            Some(&"Springfield High".to_string())
        );
        assert_eq!(metadata.get("class"), Some(&"Year 1".to_string()));

        // Array becomes the data rows
        assert_eq!(ir.header.root_key, Some("students".to_string()));
        assert_eq!(ir.header.row_count, 2);
        assert_eq!(ir.header.fields.len(), 1);
        assert_eq!(ir.header.fields[0].name, "id");
    }

    #[test]
    fn test_metadata_with_null() {
        let input = r#"{"note": null, "total": 2, "users": [{"id": 1}, {"id": 2}]}"#;
        let ir = JsonParser::parse(input).unwrap();

        // Should extract metadata including null
        assert!(ir.header.metadata.is_some());
        let metadata = ir.header.metadata.as_ref().unwrap();
        assert_eq!(metadata.get("note"), Some(&"∅".to_string()));
        assert_eq!(metadata.get("total"), Some(&"2".to_string()));

        // Array data
        assert_eq!(ir.header.root_key, Some("users".to_string()));
        assert_eq!(ir.header.row_count, 2);
        assert_eq!(ir.header.fields.len(), 1);
        assert_eq!(ir.header.fields[0].name, "id");
    }
}
