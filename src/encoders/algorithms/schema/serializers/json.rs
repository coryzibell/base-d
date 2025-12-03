use crate::encoders::algorithms::schema::fiche::NEST_SEP;
use crate::encoders::algorithms::schema::serializers::OutputSerializer;
use crate::encoders::algorithms::schema::types::*;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

pub struct JsonSerializer;

impl OutputSerializer for JsonSerializer {
    type Error = SchemaError;

    fn serialize(ir: &IntermediateRepresentation, pretty: bool) -> Result<String, Self::Error> {
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
        let result = if ir.header.row_count == 1 && ir.header.metadata.is_none() {
            // Single row without metadata - output as object
            unflattened_rows.into_iter().next().unwrap()
        } else {
            // Multiple rows OR single row with metadata - output as array
            Value::Array(unflattened_rows)
        };

        // Apply root key and metadata if present
        let final_result = if let Some(root_key) = &ir.header.root_key {
            let mut obj = Map::new();

            // Add metadata fields first (if present)
            if let Some(ref metadata) = ir.header.metadata {
                for (key, value) in metadata {
                    // Convert ∅ symbol back to JSON null
                    let json_value = if value == "∅" {
                        Value::Null
                    } else {
                        // Try to parse as number, bool, or keep as string
                        if let Ok(num) = value.parse::<i64>() {
                            json!(num)
                        } else if let Ok(num) = value.parse::<f64>() {
                            json!(num)
                        } else if value == "true" {
                            json!(true)
                        } else if value == "false" {
                            json!(false)
                        } else {
                            json!(value)
                        }
                    };
                    obj.insert(key.clone(), json_value);
                }
            }

            // Add array data under root key
            obj.insert(root_key.clone(), result);
            Value::Object(obj)
        } else {
            result
        };

        // Serialize to JSON string
        if pretty {
            serde_json::to_string_pretty(&final_result)
                .map_err(|e| SchemaError::InvalidInput(format!("JSON serialization failed: {}", e)))
        } else {
            serde_json::to_string(&final_result)
                .map_err(|e| SchemaError::InvalidInput(format!("JSON serialization failed: {}", e)))
        }
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

/// Unflatten nested keys back to nested objects
fn unflatten_object(flat: HashMap<String, Value>) -> Value {
    // First pass: identify array markers (keep them for nested reconstruction)
    let mut array_paths = std::collections::HashSet::new();
    let mut array_markers = Vec::new();
    for key in flat.keys() {
        if key.ends_with("⟦⟧") {
            // This marks an array path
            let array_path = key.trim_end_matches("⟦⟧");
            array_paths.insert(array_path.to_string());
            array_markers.push(key.clone());
        }
    }

    // Second pass: group indexed fields by their array path
    // Sort array paths by length (SHORTEST first) to match outermost arrays first
    let mut sorted_array_paths: Vec<String> = array_paths.into_iter().collect();
    sorted_array_paths.sort_by_key(|a| a.len());

    let mut array_elements: HashMap<String, Vec<(usize, String, Value)>> = HashMap::new();
    let mut non_array_fields = HashMap::new();

    for (key, value) in flat {
        // Skip array markers themselves (but we've saved them)
        if key.ends_with("⟦⟧") {
            continue;
        }

        // Check if this key belongs to an array (shortest path first)
        let mut belongs_to_array = false;
        for array_path in &sorted_array_paths {
            // Special case: empty array path (root-level array)
            if array_path.is_empty() {
                // Key should be a numeric index (no prefix)
                let parts: Vec<&str> = key.split(NEST_SEP).collect();
                if let Ok(idx) = parts[0].parse::<usize>() {
                    let remaining = if parts.len() > 1 {
                        parts[1..].join(&NEST_SEP.to_string())
                    } else {
                        String::new()
                    };
                    array_elements.entry(array_path.clone()).or_default().push((
                        idx,
                        remaining,
                        value.clone(),
                    ));
                    belongs_to_array = true;
                    break;
                }
            } else {
                // Non-empty array path: match with separator
                let separator = NEST_SEP.to_string();
                let expected_prefix = format!("{}{}", array_path, separator);
                if key.starts_with(&expected_prefix) {
                    // Extract index and remaining path
                    let after_array = &key[expected_prefix.len()..];
                    let parts: Vec<&str> = after_array.split(NEST_SEP).collect();
                    if let Ok(idx) = parts[0].parse::<usize>() {
                        // This is an array element
                        let remaining = if parts.len() > 1 {
                            parts[1..].join(&NEST_SEP.to_string())
                        } else {
                            String::new()
                        };
                        array_elements.entry(array_path.clone()).or_default().push((
                            idx,
                            remaining,
                            value.clone(),
                        ));
                        belongs_to_array = true;
                        break;
                    }
                }
            }
        }

        if !belongs_to_array {
            non_array_fields.insert(key, value);
        }
    }

    // Third pass: reconstruct arrays (longest paths first = innermost arrays first)
    #[allow(clippy::type_complexity)]
    let mut array_entries: Vec<(String, Vec<(usize, String, Value)>)> =
        array_elements.into_iter().collect();
    array_entries.sort_by(|(a, _), (b, _)| b.len().cmp(&a.len()));

    for (array_path, mut elements) in array_entries {
        // Sort by index
        elements.sort_by_key(|(idx, _, _)| *idx);

        // Find max index to determine array length
        let max_idx = elements.iter().map(|(idx, _, _)| *idx).max().unwrap_or(0);
        let mut arr = vec![Value::Null; max_idx + 1];

        // Group elements by index
        let mut by_index: HashMap<usize, Vec<(String, Value)>> = HashMap::new();
        for (idx, remaining, value) in elements {
            by_index.entry(idx).or_default().push((remaining, value));
        }

        // Build array elements
        for (idx, fields) in by_index {
            if fields.len() == 1 && fields[0].0.is_empty() {
                // Simple value
                arr[idx] = fields[0].1.clone();
            } else {
                // Nested object - reconstruct with relevant array markers
                let mut obj_map = HashMap::new();
                for (remaining, value) in fields {
                    // Skip null values when building objects
                    if !value.is_null() {
                        obj_map.insert(remaining, value);
                    }
                }

                // Include array markers that apply to this nested context
                let nested_elem_path = if array_path.is_empty() {
                    idx.to_string()
                } else {
                    format!("{}{}{}", array_path, NEST_SEP, idx)
                };
                let nested_prefix_with_sep = format!("{}{}", nested_elem_path, NEST_SEP);

                for marker in &array_markers {
                    if !marker.ends_with("⟦⟧") {
                        continue;
                    }

                    // Remove the "⟦⟧" suffix to get the path
                    let marker_path = marker.trim_end_matches("⟦⟧");

                    // Check if this marker applies to nested context
                    if marker_path.starts_with(&nested_prefix_with_sep) {
                        // Nested marker like deep჻0჻field⟦⟧ -> relative: field⟦⟧
                        let relative_path = &marker_path[nested_prefix_with_sep.len()..];
                        obj_map.insert(format!("{}⟦⟧", relative_path), Value::Null);
                    } else if marker_path == nested_elem_path {
                        // Marker equals nested element path: deep჻0⟦⟧ where we're building deep[0]
                        // This means the element itself is an array at the root level
                        // Add empty-path array marker
                        obj_map.insert("⟦⟧".to_string(), Value::Null);
                    }
                }

                arr[idx] = unflatten_object(obj_map);
            }
        }

        // Trim trailing nulls and empty objects from array
        while !arr.is_empty() {
            let last = &arr[arr.len() - 1];
            let should_remove = last.is_null()
                || (last.is_object() && last.as_object().is_some_and(|o| o.is_empty()));
            if should_remove {
                arr.pop();
            } else {
                break;
            }
        }

        non_array_fields.insert(array_path, Value::Array(arr));
    }

    // Handle empty arrays - markers with no indexed fields
    // Check which arrays actually got reconstructed
    let reconstructed_arrays: std::collections::HashSet<String> = non_array_fields
        .keys()
        .filter(|k| non_array_fields.get(*k).is_some_and(|v| v.is_array()))
        .cloned()
        .collect();

    // For arrays that have markers but weren't reconstructed, create empty arrays
    for array_path in &sorted_array_paths {
        if !reconstructed_arrays.contains(array_path) && !non_array_fields.contains_key(array_path)
        {
            // Check if this is nested inside another array element
            // If so, don't insert - it will be handled by recursive unflatten_object calls
            let is_nested_in_array = sorted_array_paths.iter().any(|parent| {
                if parent.len() >= array_path.len() {
                    return false;
                }
                let prefix = if parent.is_empty() {
                    String::new()
                } else {
                    format!("{}{}", parent, NEST_SEP)
                };
                if !array_path.starts_with(&prefix) {
                    return false;
                }
                let after = if prefix.is_empty() {
                    array_path.as_str()
                } else {
                    &array_path[prefix.len()..]
                };
                after
                    .split(NEST_SEP)
                    .next()
                    .unwrap_or("")
                    .parse::<usize>()
                    .is_ok()
            });

            if !is_nested_in_array {
                non_array_fields.insert(array_path.clone(), Value::Array(vec![]));
            }
        }
    }

    // Fourth pass: build final object
    // Special case: if there's only one field with empty key, return it directly
    if non_array_fields.len() == 1 && non_array_fields.contains_key("") {
        return non_array_fields.into_iter().next().unwrap().1;
    }

    let mut result = Map::new();
    for (key, value) in non_array_fields {
        let parts: Vec<&str> = key.split(NEST_SEP).collect();
        insert_nested_simple(&mut result, &parts, value);
    }

    Value::Object(result)
}

/// Insert a value into nested structure (simple version without array handling)
fn insert_nested_simple(obj: &mut Map<String, Value>, parts: &[&str], value: Value) {
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
        insert_nested_simple(nested_obj, remaining, value);
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

        let output = JsonSerializer::serialize(&ir, false).unwrap();
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

        let output = JsonSerializer::serialize(&ir, false).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert!(parsed.is_array());
        assert_eq!(parsed[0]["id"], json!(1));
        assert_eq!(parsed[1]["id"], json!(2));
    }

    #[test]
    fn test_nested_object() {
        let fields = vec![FieldDef::new("user჻profile჻name", FieldType::String)];
        let header = SchemaHeader::new(1, fields);
        let values = vec![SchemaValue::String("alice".to_string())];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir, false).unwrap();
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

        let output = JsonSerializer::serialize(&ir, false).unwrap();
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

        let output = JsonSerializer::serialize(&ir, false).unwrap();
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

        let output = JsonSerializer::serialize(&ir, false).unwrap();
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

        let output = JsonSerializer::serialize(&ir, false).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["items"], json!([]));
    }

    #[test]
    fn test_deep_nesting() {
        let fields = vec![FieldDef::new("a჻b჻c჻d", FieldType::U64)];
        let header = SchemaHeader::new(1, fields);
        let values = vec![SchemaValue::U64(1)];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir, false).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["a"]["b"]["c"]["d"], json!(1));
    }

    #[test]
    fn test_unflatten_object() {
        let mut flat = HashMap::new();
        flat.insert("a჻b".to_string(), json!(1));

        let unflattened = unflatten_object(flat);

        assert_eq!(unflattened["a"]["b"], json!(1));
    }

    #[test]
    fn test_pretty_output() {
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

        // Test compact output
        let compact = JsonSerializer::serialize(&ir, false).unwrap();
        assert!(!compact.contains('\n'));
        assert_eq!(compact, r#"{"id":1,"name":"alice"}"#);

        // Test pretty output
        let pretty = JsonSerializer::serialize(&ir, true).unwrap();
        assert!(pretty.contains('\n'));
        assert!(pretty.contains("  ")); // Indentation

        // Both should parse to same JSON value
        let compact_value: Value = serde_json::from_str(&compact).unwrap();
        let pretty_value: Value = serde_json::from_str(&pretty).unwrap();
        assert_eq!(compact_value, pretty_value);
    }

    #[test]
    fn test_metadata_with_null() {
        use std::collections::HashMap;

        let fields = vec![FieldDef::new("id", FieldType::U64)];
        let mut header = SchemaHeader::new(2, fields);
        header.root_key = Some("users".to_string());
        header.set_flag(FLAG_HAS_ROOT_KEY);

        let mut metadata = HashMap::new();
        metadata.insert("note".to_string(), "∅".to_string());
        metadata.insert("total".to_string(), "2".to_string());
        header.metadata = Some(metadata);

        let values = vec![SchemaValue::U64(1), SchemaValue::U64(2)];
        let ir = IntermediateRepresentation::new(header, values).unwrap();

        let output = JsonSerializer::serialize(&ir, false).unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        // Check metadata was reconstructed
        assert_eq!(parsed["note"], Value::Null);
        assert_eq!(parsed["total"], json!(2));

        // Check array data
        assert!(parsed["users"].is_array());
        assert_eq!(parsed["users"][0]["id"], json!(1));
        assert_eq!(parsed["users"][1]["id"], json!(2));
    }
}
