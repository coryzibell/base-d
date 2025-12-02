use crate::encoders::algorithms::schema::serializers::OutputSerializer;
use crate::encoders::algorithms::schema::types::*;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

pub struct JsonSerializer;

impl OutputSerializer for JsonSerializer {
    type Error = SchemaError;

    fn serialize(ir: &IntermediateRepresentation) -> Result<String, Self::Error> {
        if ir.header.row_count == 0 {
            return Err(SchemaError::InvalidInput(
                "No rows to serialize".to_string(),
            ));
        }

        // Build rows
        let mut rows = Vec::new();
        for row_idx in 0..ir.header.row_count {
            let mut row_map = HashMap::new();

            for (field_idx, field) in ir.header.fields.iter().enumerate() {
                let value = ir
                    .get_value(row_idx, field_idx)
                    .ok_or_else(|| SchemaError::InvalidInput("Missing value".to_string()))?;

                let json_value = if ir.is_null(row_idx, field_idx) {
                    Value::Null
                } else {
                    schema_value_to_json(value)?
                };

                row_map.insert(field.name.clone(), json_value);
            }

            rows.push(row_map);
        }

        // Unflatten each row
        let mut unflattened_rows = Vec::new();
        for row_map in rows {
            let unflattened = unflatten_object(row_map);
            unflattened_rows.push(unflattened);
        }

        // Determine output format
        let result = if ir.header.row_count == 1 {
            // Single row - output as object
            unflattened_rows.into_iter().next().unwrap()
        } else {
            // Multiple rows - output as array
            Value::Array(unflattened_rows)
        };

        // Apply root key if present
        let final_result = if let Some(root_key) = &ir.header.root_key {
            let mut obj = Map::new();
            obj.insert(root_key.clone(), result);
            Value::Object(obj)
        } else {
            result
        };

        // Serialize to JSON string
        serde_json::to_string(&final_result)
            .map_err(|e| SchemaError::InvalidInput(format!("JSON serialization failed: {}", e)))
    }
}

/// Convert SchemaValue to JSON Value
fn schema_value_to_json(value: &SchemaValue) -> Result<Value, SchemaError> {
    match value {
        SchemaValue::U64(n) => Ok(json!(*n)),
        SchemaValue::I64(n) => Ok(json!(*n)),
        SchemaValue::F64(n) => Ok(json!(*n)),
        SchemaValue::String(s) => Ok(json!(s)),
        SchemaValue::Bool(b) => Ok(json!(*b)),
        SchemaValue::Null => Ok(Value::Null),
        SchemaValue::Array(arr) => {
            let mut json_arr = Vec::new();
            for item in arr {
                json_arr.push(schema_value_to_json(item)?);
            }
            Ok(Value::Array(json_arr))
        }
    }
}

/// Unflatten dotted keys back to nested objects
fn unflatten_object(flat: HashMap<String, Value>) -> Value {
    let mut result = Map::new();

    for (key, value) in flat {
        let parts: Vec<&str> = key.split('.').collect();
        insert_nested(&mut result, &parts, value);
    }

    Value::Object(result)
}

/// Insert a value into nested structure
fn insert_nested(obj: &mut Map<String, Value>, parts: &[&str], value: Value) {
    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        obj.insert(parts[0].to_string(), value);
        return;
    }

    let key = parts[0];
    let remaining = &parts[1..];

    let nested = obj
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));

    if let Value::Object(nested_obj) = nested {
        insert_nested(nested_obj, remaining, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_object() {
        let fields = vec![
            FieldDef::new("id", FieldType::U64),
            FieldDef::new("name", FieldType::String),
        ];
        let header = SchemaHeader::new(1, fields);
        let values = vec![
            SchemaValue::U64(1),
            SchemaValue::String("alice".to_string()),
        ];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["id"], json!(1));
        assert_eq!(parsed["name"], json!("alice"));
    }

    #[test]
    fn test_array_of_objects() {
        let fields = vec![FieldDef::new("id", FieldType::U64)];
        let header = SchemaHeader::new(2, fields);
        let values = vec![SchemaValue::U64(1), SchemaValue::U64(2)];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert!(parsed.is_array());
        assert_eq!(parsed[0]["id"], json!(1));
        assert_eq!(parsed[1]["id"], json!(2));
    }

    #[test]
    fn test_nested_object() {
        let fields = vec![FieldDef::new("user.profile.name", FieldType::String)];
        let header = SchemaHeader::new(1, fields);
        let values = vec![SchemaValue::String("alice".to_string())];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["user"]["profile"]["name"], json!("alice"));
    }

    #[test]
    fn test_root_key() {
        let mut header = SchemaHeader::new(1, vec![FieldDef::new("id", FieldType::U64)]);
        header.root_key = Some("users".to_string());
        header.set_flag(FLAG_HAS_ROOT_KEY);

        let values = vec![SchemaValue::U64(1)];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert!(parsed["users"].is_object());
        assert_eq!(parsed["users"]["id"], json!(1));
    }

    #[test]
    fn test_null_handling() {
        let mut header = SchemaHeader::new(
            1,
            vec![
                FieldDef::new("name", FieldType::String),
                FieldDef::new("age", FieldType::U64),
            ],
        );

        // Mark age as null
        let mut null_bitmap = vec![0u8; 1];
        null_bitmap[0] |= 1 << 1; // Set bit 1
        header.null_bitmap = Some(null_bitmap);
        header.set_flag(FLAG_HAS_NULLS);

        let values = vec![SchemaValue::String("alice".to_string()), SchemaValue::Null];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["name"], json!("alice"));
        assert_eq!(parsed["age"], Value::Null);
    }

    #[test]
    fn test_homogeneous_array() {
        let fields = vec![FieldDef::new(
            "scores",
            FieldType::Array(Box::new(FieldType::U64)),
        )];
        let header = SchemaHeader::new(1, fields);
        let values = vec![SchemaValue::Array(vec![
            SchemaValue::U64(1),
            SchemaValue::U64(2),
            SchemaValue::U64(3),
        ])];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["scores"], json!([1, 2, 3]));
    }

    #[test]
    fn test_empty_array() {
        let fields = vec![FieldDef::new(
            "items",
            FieldType::Array(Box::new(FieldType::Null)),
        )];
        let header = SchemaHeader::new(1, fields);
        let values = vec![SchemaValue::Array(vec![])];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["items"], json!([]));
    }

    #[test]
    fn test_deep_nesting() {
        let fields = vec![FieldDef::new("a.b.c.d", FieldType::U64)];
        let header = SchemaHeader::new(1, fields);
        let values = vec![SchemaValue::U64(1)];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["a"]["b"]["c"]["d"], json!(1));
    }

    #[test]
    fn test_unflatten_object() {
        let mut flat = HashMap::new();
        flat.insert("a.b".to_string(), json!(1));

        let unflattened = unflatten_object(flat);

        assert_eq!(unflattened["a"]["b"], json!(1));
    }
}
