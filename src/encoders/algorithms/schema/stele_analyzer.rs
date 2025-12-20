use serde_json::Value;

/// Detected stele mode based on JSON structure
#[derive(Debug, Clone, Copy)]
pub enum DetectedMode {
    Full,
    Path,
}

/// Auto-detect the best stele mode for the given JSON structure
pub fn detect_stele_mode(json: &str) -> DetectedMode {
    let value: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return DetectedMode::Full, // Default on parse failure
    };

    let analysis = analyze_structure(&value, 0);

    // Decision heuristics:
    // 1. If schema explosion detected (>50 unique paths) → Path (most reliable signal)
    // 2. If deep nesting (>3 levels) + has indexed arrays → Path
    // 3. If varying structure in arrays → Path (schema explosion risk)
    // 4. If root is homogeneous array of objects → Full (tabular data)
    // 5. Default → Full

    // Check schema explosion FIRST - this is the most reliable signal
    if analysis.unique_paths > 50 {
        return DetectedMode::Path;
    }

    // Deep nesting with indexed arrays
    if analysis.max_depth > 3 && analysis.has_indexed_arrays {
        return DetectedMode::Path;
    }

    // Varying structure is a strong signal for path mode
    if analysis.has_varying_array_structure {
        return DetectedMode::Path;
    }

    // Homogeneous arrays work well with full mode
    if is_homogeneous_array(&value) {
        return DetectedMode::Full;
    }

    // Default to full for typical structured data
    DetectedMode::Full
}

#[derive(Default)]
struct StructureAnalysis {
    max_depth: usize,
    unique_paths: usize,
    has_indexed_arrays: bool,
    has_varying_array_structure: bool,
}

fn analyze_structure(value: &Value, depth: usize) -> StructureAnalysis {
    let mut analysis = StructureAnalysis {
        max_depth: depth,
        ..Default::default()
    };

    match value {
        Value::Object(map) => {
            let mut paths = 0;
            for (_, v) in map {
                let child = analyze_structure(v, depth + 1);
                analysis.max_depth = analysis.max_depth.max(child.max_depth);
                paths += child.unique_paths.max(1);
                analysis.has_indexed_arrays |= child.has_indexed_arrays;
                analysis.has_varying_array_structure |= child.has_varying_array_structure;
            }
            analysis.unique_paths = paths;
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                analysis.unique_paths = 1;
                return analysis;
            }

            // Check for homogeneity in arrays
            let first_type = type_signature(&arr[0]);
            let mut max_child_analysis = StructureAnalysis::default();

            // For arrays of objects, check key consistency
            if matches!(arr[0], Value::Object(_)) {
                let first_keys = if let Value::Object(map) = &arr[0] {
                    map.keys().collect::<Vec<_>>()
                } else {
                    vec![]
                };

                for item in arr.iter().skip(1) {
                    if type_signature(item) != first_type {
                        analysis.has_varying_array_structure = true;
                    }

                    // Check if keys match
                    if let Value::Object(map) = item {
                        let keys = map.keys().collect::<Vec<_>>();
                        if keys != first_keys {
                            analysis.has_varying_array_structure = true;
                        }
                    }
                }
            } else {
                // For non-object arrays, just check type
                for item in arr.iter().skip(1) {
                    if type_signature(item) != first_type {
                        analysis.has_varying_array_structure = true;
                    }
                }
            }

            // Analyze all children
            for item in arr {
                let child = analyze_structure(item, depth + 1);
                max_child_analysis.max_depth = max_child_analysis.max_depth.max(child.max_depth);
                max_child_analysis.unique_paths =
                    max_child_analysis.unique_paths.max(child.unique_paths);
                max_child_analysis.has_indexed_arrays |= child.has_indexed_arrays;
                max_child_analysis.has_varying_array_structure |= child.has_varying_array_structure;
            }

            // Any array of objects creates indexed paths
            if matches!(arr[0], Value::Object(_)) {
                analysis.has_indexed_arrays = true;
            }

            // If array of objects → paths multiply by array length
            if matches!(arr[0], Value::Object(_)) {
                analysis.unique_paths = max_child_analysis.unique_paths * arr.len();
            } else {
                analysis.unique_paths = 1;
            }

            analysis.max_depth = max_child_analysis.max_depth;
            analysis.has_indexed_arrays |= max_child_analysis.has_indexed_arrays;
            analysis.has_varying_array_structure |= max_child_analysis.has_varying_array_structure;
        }
        _ => {
            analysis.unique_paths = 1;
        }
    }

    analysis
}

fn type_signature(value: &Value) -> &str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn is_homogeneous_array(value: &Value) -> bool {
    match value {
        Value::Array(arr) => {
            if arr.is_empty() {
                return false;
            }

            // Check if all elements are objects with same keys
            let first = match &arr[0] {
                Value::Object(map) => map,
                _ => return false,
            };

            let first_keys: Vec<_> = first.keys().collect();

            for item in arr.iter().skip(1) {
                match item {
                    Value::Object(map) => {
                        let keys: Vec<_> = map.keys().collect();
                        if keys.len() != first_keys.len() {
                            return false;
                        }
                        for key in &first_keys {
                            if !keys.contains(key) {
                                return false;
                            }
                        }
                    }
                    _ => return false,
                }
            }
            true
        }
        Value::Object(map) => {
            // Check for wrapper keys like "results", "data", etc.
            if map.len() == 1 {
                for (key, value) in map {
                    if matches!(
                        key.as_str(),
                        "results" | "data" | "items" | "records" | "rows"
                    ) {
                        return is_homogeneous_array(value);
                    }
                }
            }
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_homogeneous_array() {
        let json = r#"[{"name":"alice"},{"name":"bob"}]"#;
        let mode = detect_stele_mode(json);
        assert!(matches!(mode, DetectedMode::Full));
    }

    #[test]
    fn test_detect_deep_nested() {
        // Depth: a(1) → b(2) → c(3) → d(4) → e(5) → array items(6)
        // Should trigger path mode due to depth > 4 + indexed arrays
        let json = r#"{"a":{"b":{"c":{"d":{"e":[{"f":1},{"f":2}]}}}}}"#;
        let mode = detect_stele_mode(json);
        assert!(matches!(mode, DetectedMode::Path));
    }

    #[test]
    fn test_detect_varying_structure() {
        let json = r#"{"items":[{"type":"a","x":1},{"type":"b","y":2}]}"#;
        let mode = detect_stele_mode(json);
        // Should detect varying structure
        assert!(matches!(mode, DetectedMode::Path));
    }

    #[test]
    fn test_detect_simple_object() {
        let json = r#"{"id":1,"name":"alice"}"#;
        let mode = detect_stele_mode(json);
        assert!(matches!(mode, DetectedMode::Full));
    }

    #[test]
    fn test_detect_wrapper_key() {
        let json = r#"{"results":[{"id":1},{"id":2}]}"#;
        let mode = detect_stele_mode(json);
        assert!(matches!(mode, DetectedMode::Full));
    }
}
