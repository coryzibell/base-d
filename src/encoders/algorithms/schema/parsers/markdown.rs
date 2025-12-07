use crate::encoders::algorithms::schema::parsers::InputParser;
use crate::encoders::algorithms::schema::types::*;

pub struct MarkdownParser;

impl InputParser for MarkdownParser {
    type Error = SchemaError;

    fn parse(input: &str) -> Result<IntermediateRepresentation, Self::Error> {
        // Split input into lines
        let lines: Vec<&str> = input.lines().collect();

        if lines.is_empty() {
            return Err(SchemaError::InvalidInput(
                "Empty markdown table - no content to parse.".to_string(),
            ));
        }

        if lines.len() < 2 {
            return Err(SchemaError::InvalidInput(
                "Invalid markdown table - requires header row and separator row.".to_string(),
            ));
        }

        // Parse header row (first line)
        let header_row = lines[0];
        let field_names = parse_table_row(header_row)?;

        if field_names.is_empty() {
            return Err(SchemaError::InvalidInput(
                "Empty header row - no field names found.".to_string(),
            ));
        }

        // Validate separator row (second line)
        let separator_row = lines[1];
        validate_separator_row(separator_row, field_names.len())?;

        // Parse data rows (remaining lines)
        let data_rows: Vec<Vec<String>> = lines[2..]
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| parse_table_row(line))
            .collect::<Result<Vec<_>, _>>()?;

        if data_rows.is_empty() {
            return Err(SchemaError::InvalidInput(
                "Empty table - no data rows found after header and separator.".to_string(),
            ));
        }

        let row_count = data_rows.len();

        // Infer field types from data
        let mut fields = Vec::new();
        let mut has_nulls = false;

        for (field_idx, field_name) in field_names.iter().enumerate() {
            let field_type = infer_column_type(&data_rows, field_idx, &mut has_nulls)?;
            fields.push(FieldDef::new(field_name.clone(), field_type));
        }

        // Build values and null bitmap
        let mut values = Vec::new();
        let total_values = row_count * fields.len();
        let bitmap_bytes = total_values.div_ceil(8);
        let mut null_bitmap = vec![0u8; bitmap_bytes];

        for (row_idx, row) in data_rows.iter().enumerate() {
            for (field_idx, field) in fields.iter().enumerate() {
                let value_idx = row_idx * fields.len() + field_idx;

                // Get cell value, treating missing cells as empty string
                let cell = row.get(field_idx).map(|s| s.as_str()).unwrap_or("");

                // Empty cells are null
                if cell.is_empty() {
                    values.push(SchemaValue::Null);
                    set_null_bit(&mut null_bitmap, value_idx);
                    has_nulls = true;
                } else {
                    // Parse value according to field type
                    values.push(parse_cell_value(cell, &field.field_type)?);
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
}

/// Parse a markdown table row into cells
fn parse_table_row(line: &str) -> Result<Vec<String>, SchemaError> {
    let trimmed = line.trim();

    // Remove leading and trailing pipe
    let without_pipes = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed)
        .strip_suffix('|')
        .unwrap_or(trimmed);

    // Split by pipe and trim each cell
    let cells: Vec<String> = without_pipes
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect();

    Ok(cells)
}

/// Validate the separator row has correct format
fn validate_separator_row(line: &str, expected_columns: usize) -> Result<(), SchemaError> {
    let cells = parse_table_row(line)?;

    if cells.len() != expected_columns {
        return Err(SchemaError::InvalidInput(format!(
            "Separator row has {} columns but header has {}.",
            cells.len(),
            expected_columns
        )));
    }

    // Check each cell is a valid separator (dashes with optional colons)
    for (idx, cell) in cells.iter().enumerate() {
        if !is_valid_separator_cell(cell) {
            return Err(SchemaError::InvalidInput(format!(
                "Invalid separator cell at column {}: '{}'. Expected dashes (---) with optional colons for alignment.",
                idx + 1,
                cell
            )));
        }
    }

    Ok(())
}

/// Check if a cell is a valid separator (dashes with optional colons)
fn is_valid_separator_cell(cell: &str) -> bool {
    if cell.is_empty() {
        return false;
    }

    // Valid patterns: ---, :---, ---:, :---:
    // Must have at least one dash
    let has_dash = cell.chars().any(|c| c == '-');
    let all_valid_chars = cell.chars().all(|c| c == '-' || c == ':');

    has_dash && all_valid_chars
}

/// Infer the type of a column from all its values
fn infer_column_type(
    rows: &[Vec<String>],
    column_idx: usize,
    has_nulls: &mut bool,
) -> Result<FieldType, SchemaError> {
    let mut inferred_type: Option<FieldType> = None;

    for row in rows {
        let cell = row.get(column_idx).map(|s| s.as_str()).unwrap_or("");

        // Skip empty cells (they're nulls)
        if cell.is_empty() {
            *has_nulls = true;
            continue;
        }

        let cell_type = infer_cell_type(cell);

        if let Some(ref existing_type) = inferred_type {
            // Special case: U64 and I64 unify to I64
            if (*existing_type == FieldType::U64 && cell_type == FieldType::I64)
                || (*existing_type == FieldType::I64 && cell_type == FieldType::U64)
            {
                inferred_type = Some(FieldType::I64);
                continue;
            }

            if *existing_type != cell_type {
                // Type conflict - use Any
                return Ok(FieldType::Any);
            }
        } else {
            inferred_type = Some(cell_type);
        }
    }

    Ok(inferred_type.unwrap_or(FieldType::Null))
}

/// Infer the type of a single cell value
fn infer_cell_type(cell: &str) -> FieldType {
    // Try boolean (case-insensitive)
    let lower = cell.to_lowercase();
    if lower == "true" || lower == "false" {
        return FieldType::Bool;
    }

    // Try integer
    if let Ok(num) = cell.parse::<i64>() {
        return if num < 0 {
            FieldType::I64
        } else {
            FieldType::U64
        };
    }

    // Try float
    if cell.parse::<f64>().is_ok() {
        return FieldType::F64;
    }

    // Default to string
    FieldType::String
}

/// Parse a cell value according to its expected type
fn parse_cell_value(cell: &str, field_type: &FieldType) -> Result<SchemaValue, SchemaError> {
    match field_type {
        FieldType::Bool => {
            let lower = cell.to_lowercase();
            if lower == "true" {
                Ok(SchemaValue::Bool(true))
            } else if lower == "false" {
                Ok(SchemaValue::Bool(false))
            } else {
                Err(SchemaError::InvalidInput(format!(
                    "Invalid boolean value: '{}'. Expected 'true' or 'false'.",
                    cell
                )))
            }
        }
        FieldType::U64 => {
            let num = cell.parse::<u64>().map_err(|_| {
                SchemaError::InvalidInput(format!(
                    "Invalid unsigned integer: '{}'. Value must be a non-negative integer.",
                    cell
                ))
            })?;
            Ok(SchemaValue::U64(num))
        }
        FieldType::I64 => {
            let num = cell.parse::<i64>().map_err(|_| {
                SchemaError::InvalidInput(format!(
                    "Invalid signed integer: '{}'. Value must be an integer.",
                    cell
                ))
            })?;
            Ok(SchemaValue::I64(num))
        }
        FieldType::F64 => {
            let num = cell.parse::<f64>().map_err(|_| {
                SchemaError::InvalidInput(format!(
                    "Invalid floating-point number: '{}'. Value must be a number.",
                    cell
                ))
            })?;
            Ok(SchemaValue::F64(num))
        }
        FieldType::String => Ok(SchemaValue::String(cell.to_string())),
        FieldType::Any => {
            // Try to infer and parse
            let inferred = infer_cell_type(cell);
            parse_cell_value(cell, &inferred)
        }
        FieldType::Null => Ok(SchemaValue::Null),
        _ => Err(SchemaError::InvalidInput(format!(
            "Unsupported field type for markdown parsing: {}",
            field_type.display_name()
        ))),
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
    fn test_basic_table() {
        let input = r#"| id | name  | grade |
|----| ----- |-------|
| A1 | alice | 95    |
| B2 | bob   | 87    |"#;

        let ir = MarkdownParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 2);
        assert_eq!(ir.header.fields.len(), 3);
        assert_eq!(ir.header.fields[0].name, "id");
        assert_eq!(ir.header.fields[1].name, "name");
        assert_eq!(ir.header.fields[2].name, "grade");
    }

    #[test]
    fn test_type_inference() {
        let input = r#"| int | float | bool | str |
|----|-------|------|-----|
| 42 | 3.14  | true | foo |
| -1 | 2.0   | false| bar |"#;

        let ir = MarkdownParser::parse(input).unwrap();

        assert_eq!(ir.header.fields[0].field_type, FieldType::I64); // Has negative
        assert_eq!(ir.header.fields[1].field_type, FieldType::F64);
        assert_eq!(ir.header.fields[2].field_type, FieldType::Bool);
        assert_eq!(ir.header.fields[3].field_type, FieldType::String);
    }

    #[test]
    fn test_positive_integers() {
        let input = r#"| count |
|-------|
| 1     |
| 100   |"#;

        let ir = MarkdownParser::parse(input).unwrap();

        assert_eq!(ir.header.fields[0].field_type, FieldType::U64);
    }

    #[test]
    fn test_empty_cells() {
        let input = r#"| name | age |
|------|-----|
| alice|     |
|      | 30  |"#;

        let ir = MarkdownParser::parse(input).unwrap();

        assert!(ir.header.has_flag(FLAG_HAS_NULLS));
        assert!(ir.is_null(0, 1)); // age is null in first row
        assert!(ir.is_null(1, 0)); // name is null in second row
    }

    #[test]
    fn test_alignment_markers() {
        let input = r#"| left | center | right |
|:-----|:------:|------:|
| a    | b      | c     |"#;

        let ir = MarkdownParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(ir.header.fields.len(), 3);
    }

    #[test]
    fn test_empty_table() {
        let input = r#"| id | name |
|----|------|"#;

        let result = MarkdownParser::parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_separator() {
        let input = r#"| id | name |
| XX | YY   |
| A1 | alice|"#;

        let result = MarkdownParser::parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_cells() {
        let input = r#"| a | b | c |
|---|---|---|
| 1 | 2 |
| 4 | 5 | 6 |"#;

        let ir = MarkdownParser::parse(input).unwrap();

        // First row, third column should be null
        assert!(ir.is_null(0, 2));
    }

    #[test]
    fn test_single_row() {
        let input = r#"| id |
|----|
| 42 |"#;

        let ir = MarkdownParser::parse(input).unwrap();

        assert_eq!(ir.header.row_count, 1);
        assert_eq!(ir.header.fields.len(), 1);
        assert_eq!(ir.get_value(0, 0), Some(&SchemaValue::U64(42)));
    }

    #[test]
    fn test_case_insensitive_bool() {
        let input = r#"| flag |
|------|
| TRUE |
| False|
| true |"#;

        let ir = MarkdownParser::parse(input).unwrap();

        assert_eq!(ir.header.fields[0].field_type, FieldType::Bool);
        assert_eq!(ir.get_value(0, 0), Some(&SchemaValue::Bool(true)));
        assert_eq!(ir.get_value(1, 0), Some(&SchemaValue::Bool(false)));
        assert_eq!(ir.get_value(2, 0), Some(&SchemaValue::Bool(true)));
    }
}
