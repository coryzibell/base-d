//! stele format: model-readable structured data
//!
//! stele is the model-readable sibling to carrier98. While carrier98 is opaque
//! (maximum density, model shuttles without parsing), stele uses Unicode delimiters
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
//! | `჻` | U+10FB | Nested object separator |
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

// Stele delimiters
pub const ROW_START: char = '◉'; // U+25C9 fisheye
pub const FIELD_SEP: char = '┃'; // U+2503 heavy pipe
pub const ARRAY_SEP: char = '◈'; // U+25C8 diamond in diamond
pub const NULL_VALUE: &str = "∅"; // U+2205 empty set
pub const SPACE_MARKER: char = '▓'; // U+2593 Dark Shade
pub const NEST_SEP: char = '჻'; // U+10FB Georgian paragraph separator

// Type names in stele schema (legacy)
pub const TYPE_INT: &str = "int";
pub const TYPE_STR: &str = "str";
pub const TYPE_FLOAT: &str = "float";
pub const TYPE_BOOL: &str = "bool";

// Superscript type markers (spec 1.7+)
pub const TYPE_INT_SUPER: char = 'ⁱ'; // U+2071 superscript i
pub const TYPE_STR_SUPER: char = 'ˢ'; // U+02E2 modifier letter small s
pub const TYPE_FLOAT_SUPER: char = 'ᶠ'; // U+1DA0 modifier letter small f
pub const TYPE_BOOL_SUPER: char = 'ᵇ'; // U+1D47 modifier letter small b

// Token map prefix
pub const TOKEN_MAP_PREFIX: char = '@';

// Path mode special symbols
pub const TRUE_MARKER: &str = "⊤"; // U+22A4 down tack
pub const FALSE_MARKER: &str = "⊥"; // U+22A5 up tack
pub const EMPTY_ARRAY_MARKER: &str = "⟦⟧"; // U+27E6, U+27E7 mathematical white square brackets
pub const EMPTY_OBJECT_MARKER: &str = "⟨⟩"; // U+27E8, U+27E9 mathematical angle brackets
pub const OBJECT_KEY_PREFIX: char = '#'; // Marks numeric-looking object keys (vs array indices)
pub const STRING_PREFIX: char = '"'; // Marks string values that look like numbers

/// Field name tokenization alphabet (spec 1.5+)
/// Priority: Runic (89 chars) → Hieroglyphs (1072) → Cuneiform (1024)
/// Ancient scripts avoid ASCII/digit collision and regex interference
pub mod tokens {
    /// Runic alphabet (U+16A0 – U+16F8, 89 characters, BMP)
    /// Primary tokenization alphabet - most compact, BMP-only
    pub const RUNIC: &[char] = &[
        'ᚠ', 'ᚡ', 'ᚢ', 'ᚣ', 'ᚤ', 'ᚥ', 'ᚦ', 'ᚧ', 'ᚨ', 'ᚩ', 'ᚪ', 'ᚫ', 'ᚬ', 'ᚭ', 'ᚮ', 'ᚯ', 'ᚰ', 'ᚱ',
        'ᚲ', 'ᚳ', 'ᚴ', 'ᚵ', 'ᚶ', 'ᚷ', 'ᚸ', 'ᚹ', 'ᚺ', 'ᚻ', 'ᚼ', 'ᚽ', 'ᚾ', 'ᚿ', 'ᛀ', 'ᛁ', 'ᛂ', 'ᛃ',
        'ᛄ', 'ᛅ', 'ᛆ', 'ᛇ', 'ᛈ', 'ᛉ', 'ᛊ', 'ᛋ', 'ᛌ', 'ᛍ', 'ᛎ', 'ᛏ', 'ᛐ', 'ᛑ', 'ᛒ', 'ᛓ', 'ᛔ', 'ᛕ',
        'ᛖ', 'ᛗ', 'ᛘ', 'ᛙ', 'ᛚ', 'ᛛ', 'ᛜ', 'ᛝ', 'ᛞ', 'ᛟ', 'ᛠ', 'ᛡ', 'ᛢ', 'ᛣ', 'ᛤ', 'ᛥ', 'ᛦ', 'ᛧ',
        'ᛨ', 'ᛩ', 'ᛪ', '᛫', '᛬', '᛭', 'ᛮ', 'ᛯ', 'ᛰ', 'ᛱ', 'ᛲ', 'ᛳ', 'ᛴ', 'ᛵ', 'ᛶ', 'ᛷ', 'ᛸ',
    ];

    /// Get a token character by index, spanning all alphabets
    /// Returns None if index exceeds available tokens
    pub fn get_token(index: usize) -> Option<char> {
        if index < RUNIC.len() {
            Some(RUNIC[index])
        } else {
            // Hieroglyphs and Cuneiform would be added here for overflow
            // For now, return None if we exceed runic capacity
            None
        }
    }

    /// Check if a character is a valid token
    pub fn is_token(c: char) -> bool {
        RUNIC.contains(&c)
    }

    /// Get the index of a token character
    #[allow(dead_code)]
    pub fn token_index(c: char) -> Option<usize> {
        RUNIC.iter().position(|&t| t == c)
    }
}

/// Value tokenization alphabet (spec 1.8+)
/// Egyptian Hieroglyphs for repeated value compression
pub mod value_tokens {
    /// Hieroglyphs (U+13000 – U+1342F, 1072 characters, SMP)
    /// Used for value tokenization to avoid collision with field tokens
    pub const HIEROGLYPH_START: char = '\u{13000}'; // 𓀀
    pub const HIEROGLYPH_END: char = '\u{1342F}'; // 1072 chars available

    /// Get a hieroglyph token by index
    pub fn get_token(index: usize) -> Option<char> {
        let code_point = HIEROGLYPH_START as u32 + index as u32;
        if code_point <= HIEROGLYPH_END as u32 {
            char::from_u32(code_point)
        } else {
            None
        }
    }

    /// Check if a character is a hieroglyph token
    pub fn is_token(c: char) -> bool {
        (HIEROGLYPH_START..=HIEROGLYPH_END).contains(&c)
    }

    /// Get the index of a hieroglyph token
    #[allow(dead_code)]
    pub fn token_index(c: char) -> Option<usize> {
        if is_token(c) {
            Some((c as u32 - HIEROGLYPH_START as u32) as usize)
        } else {
            None
        }
    }
}

/// Serialize IR to stele format (with tokenization)
pub fn serialize(ir: &IntermediateRepresentation, minify: bool) -> Result<String, SchemaError> {
    serialize_full_options(ir, minify, true, true)
}

/// Serialize IR to stele format without tokenization (human-readable field names)
pub fn serialize_readable(
    ir: &IntermediateRepresentation,
    minify: bool,
) -> Result<String, SchemaError> {
    serialize_full_options(ir, minify, false, false)
}

/// Serialize IR with field tokenization only (no value dictionary)
pub fn serialize_light(
    ir: &IntermediateRepresentation,
    minify: bool,
) -> Result<String, SchemaError> {
    serialize_full_options(ir, minify, true, false)
}

#[allow(dead_code)]
pub fn serialize_minified(ir: &IntermediateRepresentation) -> Result<String, SchemaError> {
    serialize_full_options(ir, true, true, true)
}

/// Serialize with minify, tokenization, and value dictionary options
fn serialize_full_options(
    ir: &IntermediateRepresentation,
    minify: bool,
    tokenize: bool,
    tokenize_values: bool,
) -> Result<String, SchemaError> {
    // For backward compatibility, delegate to old function if not tokenizing
    if !tokenize {
        return serialize_with_options(ir, minify);
    }

    let mut output = String::new();
    let line_sep = if minify { SPACE_MARKER } else { '\n' };

    // Build token map for field names
    let mut token_map: Vec<(char, &str)> = Vec::new();
    for (idx, field) in ir.header.fields.iter().enumerate() {
        if let Some(token) = tokens::get_token(idx) {
            token_map.push((token, &field.name));
        } else {
            // Fall back to non-tokenized if we run out of tokens
            return serialize_with_options(ir, minify);
        }
    }

    // Build value dictionary if tokenize_values is enabled
    let value_dict = if tokenize_values {
        build_value_dictionary(ir)
    } else {
        std::collections::HashMap::new()
    };

    // Field token map header: @ᚠ=field1,ᚡ=field2,...
    output.push(TOKEN_MAP_PREFIX);
    for (idx, (token, name)) in token_map.iter().enumerate() {
        if idx > 0 {
            output.push(',');
        }
        output.push(*token);
        output.push('=');
        output.push_str(name);
    }
    output.push(line_sep);

    // Value dictionary header (if present): @𓀀=value1,𓀁=value2,...
    if !value_dict.is_empty() {
        output.push(TOKEN_MAP_PREFIX);
        // Sort by token for deterministic output
        let mut sorted_values: Vec<_> = value_dict.iter().collect();
        sorted_values.sort_by_key(|(_, token)| **token);

        for (idx, (value, token)) in sorted_values.iter().enumerate() {
            if idx > 0 {
                output.push(',');
            }
            output.push(**token);
            output.push('=');
            output.push_str(value);
        }
        output.push(line_sep);
    }

    // Schema line with tokens: @{root}┃ᚠ{type}┃ᚡ{type}...
    output.push('@');
    if let Some(ref root_key) = ir.header.root_key {
        output.push_str(root_key);
    }

    // Add metadata annotation if present
    if let Some(ref metadata) = ir.header.metadata {
        output.push('[');
        let mut sorted_keys: Vec<&String> = metadata.keys().collect();
        sorted_keys.sort(); // Deterministic order for roundtrip
        for (idx, key) in sorted_keys.iter().enumerate() {
            if idx > 0 {
                output.push(',');
            }
            output.push_str(key);
            output.push('=');
            // Replace spaces with SPACE_MARKER in metadata values
            let value = metadata[*key].replace(' ', &SPACE_MARKER.to_string());
            output.push_str(&value);
        }
        output.push(']');
    }

    for (idx, field) in ir.header.fields.iter().enumerate() {
        output.push(FIELD_SEP);
        output.push(token_map[idx].0); // Token instead of field name
        // Array markers (name⟦⟧) are structural metadata - no type suffix
        if !field.name.ends_with("⟦⟧") {
            output.push_str(&field_type_to_str(&field.field_type));
        }
    }
    output.push(line_sep);

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
                let value_str = value_to_str(value, &field.field_type);

                // Replace with token if value is in dictionary
                if let Some(&token) = value_dict.get(&value_str) {
                    output.push(token);
                } else {
                    output.push_str(&value_str);
                }
            }
        }
        if row < ir.header.row_count - 1 {
            output.push(line_sep);
        }
    }

    Ok(output)
}

/// Build value dictionary from IR
/// Returns map of value string -> hieroglyph token for values appearing 2+ times
fn build_value_dictionary(
    ir: &IntermediateRepresentation,
) -> std::collections::HashMap<String, char> {
    use std::collections::HashMap;

    // Count occurrences of each string value
    let mut value_counts: HashMap<String, usize> = HashMap::new();
    let field_count = ir.header.fields.len();

    for row in 0..ir.header.row_count {
        for field_idx in 0..ir.header.fields.len() {
            // Skip nulls
            if ir.is_null(row, field_idx) {
                continue;
            }

            let value_idx = row * field_count + field_idx;
            let value = &ir.values[value_idx];

            // Only tokenize string values (exclude numbers, bools, nulls)
            if let SchemaValue::String(s) = value {
                let value_str = s.replace(' ', &SPACE_MARKER.to_string());
                *value_counts.entry(value_str).or_insert(0) += 1;
            }
        }
    }

    // Build dictionary for values with count >= 2
    let mut dict: HashMap<String, char> = HashMap::new();
    let mut sorted_values: Vec<_> = value_counts
        .iter()
        .filter(|(_, count)| **count >= 2)
        .collect();

    // Sort by frequency descending (most common first)
    sorted_values.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

    for (idx, (value, _)) in sorted_values.iter().enumerate() {
        if let Some(token) = value_tokens::get_token(idx) {
            dict.insert((*value).clone(), token);
        } else {
            // Ran out of hieroglyph tokens (unlikely with 1072 available)
            break;
        }
    }

    dict
}

fn serialize_with_options(
    ir: &IntermediateRepresentation,
    minify: bool,
) -> Result<String, SchemaError> {
    let mut output = String::new();
    let line_sep = if minify { SPACE_MARKER } else { '\n' };

    // Schema line: @{root}[meta1=val1,meta2=val2]┃{field}:{type}...
    output.push('@');
    if let Some(ref root_key) = ir.header.root_key {
        output.push_str(root_key);
    }

    // Add metadata annotation if present
    if let Some(ref metadata) = ir.header.metadata {
        output.push('[');
        let mut sorted_keys: Vec<&String> = metadata.keys().collect();
        sorted_keys.sort(); // Deterministic order for roundtrip
        for (idx, key) in sorted_keys.iter().enumerate() {
            if idx > 0 {
                output.push(',');
            }
            output.push_str(key);
            output.push('=');
            // Replace spaces with SPACE_MARKER in metadata values
            let value = metadata[*key].replace(' ', &SPACE_MARKER.to_string());
            output.push_str(&value);
        }
        output.push(']');
    }

    for field in &ir.header.fields {
        output.push(FIELD_SEP);
        output.push_str(&field.name);
        // Array markers (name⟦⟧) are structural metadata - no type suffix
        // Other fields get superscript type marker
        if !field.name.ends_with("⟦⟧") {
            output.push_str(&field_type_to_str(&field.field_type));
        }
    }
    output.push(line_sep);

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
        if row < ir.header.row_count - 1 {
            output.push(line_sep);
        }
    }

    Ok(output)
}

/// Parse stele format to IR
/// Supports both tokenized and non-tokenized formats
pub fn parse(input: &str) -> Result<IntermediateRepresentation, SchemaError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(SchemaError::InvalidInput("Empty stele input".to_string()));
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

    // Parse token maps if present (tokenized format)
    // Format: @ᚠ=field1,ᚡ=field2,...\n@𓀀=value1,𓀁=value2,...\n@root┃...
    // Or minified: @ᚠ=field1,...▓@𓀀=value1,...▓@root┃...
    let mut token_map: std::collections::HashMap<char, String> = std::collections::HashMap::new();
    let mut value_dict: std::collections::HashMap<char, String> = std::collections::HashMap::new();
    let schema_part = schema_part.trim();

    // Parse dictionary lines
    let effective_schema = {
        let lines: Vec<&str> = schema_part
            .split(['\n', SPACE_MARKER])
            .filter(|s| !s.is_empty())
            .collect();

        let mut schema_line_idx = 0;

        // Parse up to 2 dictionary lines (field dict, value dict)
        for (idx, line) in lines.iter().enumerate() {
            if !line.starts_with('@') || line.len() <= 1 {
                schema_line_idx = idx;
                break;
            }

            let after_at = &line[1..];
            let first_char = after_at.chars().next();

            if let Some(fc) = first_char {
                // Field token dictionary (runic)
                if tokens::is_token(fc) && after_at.contains('=') {
                    let map_content = after_at;
                    for pair in map_content.split(',') {
                        let parts: Vec<&str> = pair.splitn(2, '=').collect();
                        if parts.len() == 2 {
                            let token = parts[0].chars().next();
                            let name = parts[1].to_string();
                            if let Some(t) = token {
                                token_map.insert(t, name);
                            }
                        }
                    }
                    schema_line_idx = idx + 1;
                }
                // Value token dictionary (hieroglyphs)
                else if value_tokens::is_token(fc) && after_at.contains('=') {
                    let map_content = after_at;
                    for pair in map_content.split(',') {
                        let parts: Vec<&str> = pair.splitn(2, '=').collect();
                        if parts.len() == 2 {
                            let token = parts[0].chars().next();
                            let value = parts[1].to_string();
                            if let Some(t) = token {
                                value_dict.insert(t, value);
                            }
                        }
                    }
                    schema_line_idx = idx + 1;
                }
                // Schema line (not a dictionary)
                else {
                    schema_line_idx = idx;
                    break;
                }
            } else {
                schema_line_idx = idx;
                break;
            }
        }

        if schema_line_idx < lines.len() {
            lines[schema_line_idx..].join(&SPACE_MARKER.to_string())
        } else {
            return Err(SchemaError::InvalidInput(
                "No schema line found after dictionaries".to_string(),
            ));
        }
    };

    // Parse schema line (strip minified line separator if present)
    let schema_line = effective_schema.trim().trim_end_matches(SPACE_MARKER);
    if !schema_line.starts_with('@') {
        return Err(SchemaError::InvalidInput(
            "Schema line must start with @".to_string(),
        ));
    }

    let schema_content = &schema_line[1..]; // Remove @

    // Check for metadata annotation: @root[key=val,...]┃field:type...
    let (root_and_metadata, field_defs) = if let Some(sep_pos) = schema_content.find(FIELD_SEP) {
        (&schema_content[..sep_pos], &schema_content[sep_pos..])
    } else {
        return Err(SchemaError::InvalidInput(
            "Schema line must contain at least one field definition".to_string(),
        ));
    };

    // Parse root key and metadata
    let (root_key, metadata) = if let Some(bracket_start) = root_and_metadata.find('[') {
        let root = &root_and_metadata[..bracket_start];
        let root_key = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        // Find matching closing bracket (handle nested brackets in JSON values)
        let meta_content = &root_and_metadata[bracket_start + 1..];
        let mut depth = 0;
        let mut bracket_end = None;
        for (idx, ch) in meta_content.char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    if depth == 0 {
                        bracket_end = Some(idx);
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }

        if let Some(end_pos) = bracket_end {
            let meta_str = &meta_content[..end_pos];
            let mut metadata = std::collections::HashMap::new();

            // Parse key=value pairs (handle JSON arrays with commas)
            let mut current_key = String::new();
            let mut current_value = String::new();
            let mut in_value = false;
            let mut json_depth = 0;

            for ch in meta_str.chars() {
                match ch {
                    '=' if !in_value && json_depth == 0 => {
                        in_value = true;
                    }
                    '[' if in_value => {
                        json_depth += 1;
                        current_value.push(ch);
                    }
                    ']' if in_value => {
                        json_depth -= 1;
                        current_value.push(ch);
                    }
                    ',' if in_value && json_depth == 0 => {
                        // End of key=value pair
                        let key = current_key.trim().to_string();
                        let value = current_value.trim().replace(SPACE_MARKER, " ");
                        if !key.is_empty() {
                            metadata.insert(key, value);
                        }
                        current_key.clear();
                        current_value.clear();
                        in_value = false;
                    }
                    _ => {
                        if in_value {
                            current_value.push(ch);
                        } else {
                            current_key.push(ch);
                        }
                    }
                }
            }

            // Insert final pair
            if !current_key.is_empty() {
                let key = current_key.trim().to_string();
                let value = current_value.trim().replace(SPACE_MARKER, " ");
                metadata.insert(key, value);
            }

            (
                root_key,
                if metadata.is_empty() {
                    None
                } else {
                    Some(metadata)
                },
            )
        } else {
            return Err(SchemaError::InvalidInput(
                "Unclosed metadata bracket in schema".to_string(),
            ));
        }
    } else {
        // No metadata, check for root key
        let root = root_and_metadata.trim();
        let root_key = if root.is_empty() || root.contains(':') {
            None
        } else {
            Some(root.to_string())
        };
        (root_key, None)
    };

    // Parse field definitions
    let schema_parts: Vec<&str> = field_defs.split(FIELD_SEP).collect();
    let mut fields = Vec::new();

    for part in &schema_parts {
        if part.is_empty() {
            continue;
        }
        let (name, field_type) = parse_field_def_with_tokens(part, &token_map)?;
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
        // Trim whitespace and minified line separator
        let row_str = row_str.trim().trim_end_matches(SPACE_MARKER);
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
                // For array fields, ∅ means empty array, not null
                if matches!(field.field_type, FieldType::Array(_)) {
                    values.push(SchemaValue::Array(vec![]));
                } else {
                    null_positions.push(row_count * fields.len() + field_idx);
                    values.push(SchemaValue::Null);
                }
            } else {
                // Check if value is a single hieroglyph token
                let resolved_value = if value_str.len() == 1 || value_str.chars().count() == 1 {
                    let first_char = value_str.chars().next().unwrap();
                    if let Some(expanded) = value_dict.get(&first_char) {
                        expanded.as_str()
                    } else {
                        value_str
                    }
                } else {
                    value_str
                };

                let value = parse_value(resolved_value, &field.field_type)?;
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
    header.metadata = metadata;

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

/// Convert FieldType to stele type string (superscript format)
fn field_type_to_str(ft: &FieldType) -> String {
    match ft {
        FieldType::U64 | FieldType::I64 => TYPE_INT_SUPER.to_string(),
        FieldType::F64 => TYPE_FLOAT_SUPER.to_string(),
        FieldType::String => TYPE_STR_SUPER.to_string(),
        FieldType::Bool => TYPE_BOOL_SUPER.to_string(),
        FieldType::Null => TYPE_STR_SUPER.to_string(), // Nulls rendered as str type
        FieldType::Array(inner) => {
            // Inline primitive arrays: emit type⟦⟧
            let inner_str = field_type_to_str(inner);
            format!("{}⟦⟧", inner_str)
        }
        FieldType::Any => TYPE_STR_SUPER.to_string(),
    }
}

/// Parse stele type string to FieldType
/// Supports both legacy (int, str, float, bool) and superscript (ⁱ, ˢ, ᶠ, ᵇ) formats
fn parse_type_str(s: &str) -> Result<FieldType, SchemaError> {
    // Support both old [] and new ⟦⟧ syntax for backward compatibility
    if let Some(inner) = s.strip_suffix("⟦⟧").or_else(|| s.strip_suffix("[]")) {
        let inner_type = parse_type_str(inner)?;
        return Ok(FieldType::Array(Box::new(inner_type)));
    }

    // Superscript type markers (spec 1.7+)
    let type_int_super = TYPE_INT_SUPER.to_string();
    let type_str_super = TYPE_STR_SUPER.to_string();
    let type_float_super = TYPE_FLOAT_SUPER.to_string();
    let type_bool_super = TYPE_BOOL_SUPER.to_string();

    match s {
        // Legacy format
        TYPE_INT => Ok(FieldType::I64),
        TYPE_STR => Ok(FieldType::String),
        TYPE_FLOAT => Ok(FieldType::F64),
        TYPE_BOOL => Ok(FieldType::Bool),
        // Superscript format
        _ if s == type_int_super => Ok(FieldType::I64),
        _ if s == type_str_super => Ok(FieldType::String),
        _ if s == type_float_super => Ok(FieldType::F64),
        _ if s == type_bool_super => Ok(FieldType::Bool),
        // Nested content marker
        "@" => Ok(FieldType::Array(Box::new(FieldType::String))),
        _ => Err(SchemaError::InvalidInput(format!(
            "Unknown type '{}'. Valid types: int/ⁱ, str/ˢ, float/ᶠ, bool/ᵇ, @",
            s
        ))),
    }
}

/// Parse field definition with token map support
/// Resolves token characters to field names using the provided map
fn parse_field_def_with_tokens(
    s: &str,
    token_map: &std::collections::HashMap<char, String>,
) -> Result<(String, FieldType), SchemaError> {
    // Get the (possibly tokenized) name and type
    let (name_or_token, field_type) = parse_field_def(s)?;

    // Check if the name is a single-char token that needs resolution
    let chars: Vec<char> = name_or_token.chars().collect();
    if chars.len() == 1
        && let Some(resolved_name) = token_map.get(&chars[0])
    {
        return Ok((resolved_name.clone(), field_type));
    }

    // Not a token, use as-is
    Ok((name_or_token, field_type))
}

/// Parse field definition
/// Supports both formats:
/// - Legacy: "name:str", "tags:str[]", "tags:str⟦⟧"
/// - Superscript: "nameˢ", "tagsˢ⟦⟧"
/// - Tokenized: "ᚠˢ" (single runic char followed by type)
fn parse_field_def(s: &str) -> Result<(String, FieldType), SchemaError> {
    // First, try legacy format with `:` separator
    if let Some(colon_pos) = s.find(':') {
        let name = s[..colon_pos].trim().to_string();
        let field_type = parse_type_str(s[colon_pos + 1..].trim())?;
        return Ok((name, field_type));
    }

    // Superscript format: field name ends with type marker
    // Check for array suffix first (ˢ⟦⟧, ⁱ⟦⟧, etc.)
    let (base, is_array) = if let Some(stripped) = s.strip_suffix("⟦⟧") {
        (stripped, true)
    } else if let Some(stripped) = s.strip_suffix("[]") {
        (stripped, true)
    } else {
        (s, false)
    };

    // Now find the type marker at the end
    let type_markers = [
        (TYPE_STR_SUPER, FieldType::String),
        (TYPE_INT_SUPER, FieldType::I64),
        (TYPE_FLOAT_SUPER, FieldType::F64),
        (TYPE_BOOL_SUPER, FieldType::Bool),
    ];

    for (marker, field_type) in &type_markers {
        let marker_str = marker.to_string();
        if base.ends_with(&marker_str) {
            let name = base[..base.len() - marker_str.len()].trim().to_string();
            if name.is_empty() {
                return Err(SchemaError::InvalidInput(format!(
                    "Empty field name in '{}'",
                    s
                )));
            }
            let final_type = if is_array {
                FieldType::Array(Box::new(field_type.clone()))
            } else {
                field_type.clone()
            };
            return Ok((name, final_type));
        }
    }

    Err(SchemaError::InvalidInput(format!(
        "Invalid field definition '{}'. Expected format: name:type or nameˢ/ⁱ/ᶠ/ᵇ",
        s
    )))
}

/// Convert SchemaValue to stele string
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
        SchemaValue::String(s) => s.replace(' ', &SPACE_MARKER.to_string()),
        SchemaValue::Bool(b) => b.to_string(),
        SchemaValue::Null => NULL_VALUE.to_string(),
        SchemaValue::Array(arr) => {
            if arr.is_empty() {
                return NULL_VALUE.to_string();
            }

            let inner_type = if let FieldType::Array(inner) = field_type {
                inner.as_ref()
            } else {
                &FieldType::String
            };

            // Inline primitive arrays with ◈ separator
            let elements: Vec<String> = arr.iter().map(|v| value_to_str(v, inner_type)).collect();

            elements.join(&ARRAY_SEP.to_string())
        }
    }
}

/// Parse value string to SchemaValue
fn parse_value(s: &str, field_type: &FieldType) -> Result<SchemaValue, SchemaError> {
    // Check for null marker first (applies to all types)
    if s == NULL_VALUE {
        return Ok(SchemaValue::Null);
    }

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
        FieldType::String => Ok(SchemaValue::String(s.replace(SPACE_MARKER, " "))),
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
            // Split by ARRAY_SEP (◈) for inline primitive arrays
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

/// Serialize JSON to path mode format
/// Emits one line per leaf value with full path from root
pub fn serialize_path_mode(json: &str) -> Result<String, SchemaError> {
    use serde_json::Value;
    use std::collections::HashMap;

    // Parse JSON
    let value: Value = serde_json::from_str(json)
        .map_err(|e| SchemaError::InvalidInput(format!("Invalid JSON: {}", e)))?;

    // Collect all path-value pairs
    let mut path_values: Vec<(String, String)> = Vec::new();
    collect_paths(&value, String::new(), &mut path_values);

    // Collect path segment frequencies
    let mut segment_counts: HashMap<String, usize> = HashMap::new();
    for (path, _) in &path_values {
        for segment in path.split(NEST_SEP) {
            // Skip numeric array indices
            if segment.parse::<usize>().is_err() {
                *segment_counts.entry(segment.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Build path segment dictionary (runic tokens for segments appearing 2+ times)
    let mut path_dict: HashMap<String, char> = HashMap::new();
    let mut sorted_segments: Vec<_> = segment_counts
        .iter()
        .filter(|(_, count)| **count >= 2)
        .collect();
    sorted_segments.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

    for (idx, (segment, _)) in sorted_segments.iter().enumerate() {
        if let Some(token) = tokens::get_token(idx) {
            path_dict.insert((*segment).clone(), token);
        }
    }

    // Count string value frequencies for dictionary
    let mut value_counts: HashMap<String, usize> = HashMap::new();
    for (_, val) in &path_values {
        // Only count non-empty strings (exclude numbers, special markers)
        if !val.is_empty()
            && val != NULL_VALUE
            && val != TRUE_MARKER
            && val != FALSE_MARKER
            && val != EMPTY_ARRAY_MARKER
            && val != EMPTY_OBJECT_MARKER
            && !val
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
        {
            *value_counts.entry(val.clone()).or_insert(0) += 1;
        }
    }

    // Build dictionary for values appearing 2+ times (hieroglyphs)
    let mut value_dict: HashMap<String, char> = HashMap::new();
    let mut sorted_values: Vec<_> = value_counts
        .iter()
        .filter(|(_, count)| **count >= 2)
        .collect();
    sorted_values.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

    for (idx, (value, _)) in sorted_values.iter().enumerate() {
        if let Some(token) = value_tokens::get_token(idx) {
            value_dict.insert((*value).clone(), token);
        }
    }

    // Build output
    let mut output = String::new();

    // Emit path segment dictionary if present (runic)
    if !path_dict.is_empty() {
        output.push(TOKEN_MAP_PREFIX);
        let mut sorted_dict: Vec<_> = path_dict.iter().collect();
        sorted_dict.sort_by_key(|(_, token)| **token);

        for (idx, (segment, token)) in sorted_dict.iter().enumerate() {
            if idx > 0 {
                output.push(',');
            }
            output.push(**token);
            output.push('=');
            output.push_str(segment);
        }
        output.push('\n');
    }

    // Emit value dictionary if present (hieroglyphs)
    if !value_dict.is_empty() {
        output.push(TOKEN_MAP_PREFIX);
        let mut sorted_dict: Vec<_> = value_dict.iter().collect();
        sorted_dict.sort_by_key(|(_, token)| **token);

        for (idx, (value, token)) in sorted_dict.iter().enumerate() {
            if idx > 0 {
                output.push(',');
            }
            output.push(**token);
            output.push('=');
            output.push_str(value);
        }
        output.push('\n');
    }

    // Emit path-value lines with tokenized segments
    for (path, value) in path_values {
        // Tokenize path segments
        let tokenized_path = tokenize_path(&path, &path_dict);
        output.push_str(&tokenized_path);
        output.push(FIELD_SEP);

        // Replace with token if in dictionary
        if let Some(&token) = value_dict.get(&value) {
            output.push(token);
        } else {
            output.push_str(&value);
        }
        output.push('\n');
    }

    // Remove trailing newline
    if output.ends_with('\n') {
        output.pop();
    }

    Ok(output)
}

/// Replace path segments with tokens from dictionary
fn tokenize_path(path: &str, dict: &std::collections::HashMap<String, char>) -> String {
    let parts: Vec<&str> = path.split(NEST_SEP).collect();
    let tokenized_parts: Vec<String> = parts
        .iter()
        .map(|segment| {
            // Check if segment is in dictionary
            if let Some(&token) = dict.get(*segment) {
                token.to_string()
            } else {
                segment.to_string()
            }
        })
        .collect();
    tokenized_parts.join(&NEST_SEP.to_string())
}

/// Recursively collect all leaf paths and values from JSON
fn collect_paths(value: &serde_json::Value, path: String, output: &mut Vec<(String, String)>) {
    use serde_json::Value;

    match value {
        Value::Null => {
            output.push((path, NULL_VALUE.to_string()));
        }
        Value::Bool(b) => {
            let val = if *b { TRUE_MARKER } else { FALSE_MARKER };
            output.push((path, val.to_string()));
        }
        Value::Number(n) => {
            output.push((path, n.to_string()));
        }
        Value::String(s) => {
            let val = s.replace(' ', &SPACE_MARKER.to_string());
            // Mark string values that look like numbers for round-trip fidelity
            let val = if looks_like_number(&val) {
                format!("{}{}", STRING_PREFIX, val)
            } else {
                val
            };
            output.push((path, val));
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                output.push((path, EMPTY_ARRAY_MARKER.to_string()));
            } else {
                for (idx, item) in arr.iter().enumerate() {
                    let item_path = if path.is_empty() {
                        idx.to_string()
                    } else {
                        format!("{}{}{}", path, NEST_SEP, idx)
                    };
                    collect_paths(item, item_path, output);
                }
            }
        }
        Value::Object(obj) => {
            if obj.is_empty() {
                output.push((path, EMPTY_OBJECT_MARKER.to_string()));
            } else {
                for (key, val) in obj {
                    // Mark numeric-looking keys to distinguish from array indices
                    let marked_key = if key.parse::<usize>().is_ok() {
                        format!("{}{}", OBJECT_KEY_PREFIX, key)
                    } else {
                        key.clone()
                    };
                    let key_path = if path.is_empty() {
                        marked_key
                    } else {
                        format!("{}{}{}", path, NEST_SEP, marked_key)
                    };
                    collect_paths(val, key_path, output);
                }
            }
        }
    }
}

/// Check if a string looks like a number (integer or float)
fn looks_like_number(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Try to parse as integer or float
    s.parse::<i64>().is_ok() || s.parse::<f64>().is_ok()
}

/// Parse path mode format back to JSON
pub fn parse_path_mode(input: &str) -> Result<String, SchemaError> {
    use serde_json::Value;
    use std::collections::HashMap;

    let input = input.trim();
    if input.is_empty() {
        return Err(SchemaError::InvalidInput(
            "Empty path mode input".to_string(),
        ));
    }

    // Parse dictionaries if present
    let mut path_dict: HashMap<char, String> = HashMap::new();
    let mut value_dict: HashMap<char, String> = HashMap::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut data_start_idx = 0;

    // Parse up to 2 dictionary lines
    for (idx, line) in lines.iter().enumerate() {
        if !line.starts_with(TOKEN_MAP_PREFIX) {
            data_start_idx = idx;
            break;
        }

        let dict_content = &line[1..];
        if dict_content.is_empty() {
            data_start_idx = idx + 1;
            continue;
        }

        // Detect dictionary type by first token character
        let first_token = dict_content.chars().next();
        if first_token.is_none() {
            data_start_idx = idx + 1;
            continue;
        }

        let ft = first_token.unwrap();
        let is_runic = tokens::is_token(ft);
        let is_hieroglyph = value_tokens::is_token(ft);

        if is_runic {
            // Path segment dictionary (runic)
            for pair in dict_content.split(',') {
                let parts: Vec<&str> = pair.splitn(2, '=').collect();
                if parts.len() == 2
                    && let Some(token) = parts[0].chars().next()
                {
                    path_dict.insert(token, parts[1].to_string());
                }
            }
        } else if is_hieroglyph {
            // Value dictionary (hieroglyphs)
            for pair in dict_content.split(',') {
                let parts: Vec<&str> = pair.splitn(2, '=').collect();
                if parts.len() == 2
                    && let Some(token) = parts[0].chars().next()
                {
                    value_dict.insert(token, parts[1].to_string());
                }
            }
        }

        data_start_idx = idx + 1;
    }

    // Parse path-value lines into map
    let mut paths: HashMap<String, Value> = HashMap::new();

    for line in lines.iter().skip(data_start_idx) {
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, FIELD_SEP).collect();
        if parts.len() != 2 {
            return Err(SchemaError::InvalidInput(format!(
                "Invalid path line: '{}'",
                line
            )));
        }

        // Detokenize path segments
        let tokenized_path = parts[0];
        let detokenized_path = detokenize_path(tokenized_path, &path_dict);

        let mut value_str = parts[1];

        // Resolve value token if single char
        let resolved: String;
        if (value_str.len() == 1 || value_str.chars().count() == 1)
            && let Some(c) = value_str.chars().next()
            && let Some(expanded) = value_dict.get(&c)
        {
            resolved = expanded.clone();
            value_str = &resolved;
        }

        // Parse value
        let value = parse_path_value(value_str)?;
        paths.insert(detokenized_path, value);
    }

    // Build JSON from paths
    let root = build_json_from_paths(&paths)?;
    serde_json::to_string(&root)
        .map_err(|e| SchemaError::InvalidInput(format!("JSON serialization failed: {}", e)))
}

/// Replace path segment tokens with original strings from dictionary
fn detokenize_path(path: &str, dict: &std::collections::HashMap<char, String>) -> String {
    let parts: Vec<&str> = path.split(NEST_SEP).collect();
    let detokenized_parts: Vec<String> = parts
        .iter()
        .map(|segment| {
            // Check if segment is a single token character
            if (segment.len() == 1 || segment.chars().count() == 1)
                && let Some(c) = segment.chars().next()
                && let Some(expanded) = dict.get(&c)
            {
                return expanded.clone();
            }
            segment.to_string()
        })
        .collect();
    detokenized_parts.join(&NEST_SEP.to_string())
}

/// Parse a path value string to serde_json::Value
fn parse_path_value(s: &str) -> Result<serde_json::Value, SchemaError> {
    use serde_json::{Value, json};

    match s {
        NULL_VALUE => Ok(Value::Null),
        TRUE_MARKER => Ok(json!(true)),
        FALSE_MARKER => Ok(json!(false)),
        EMPTY_ARRAY_MARKER => Ok(json!([])),
        EMPTY_OBJECT_MARKER => Ok(json!({})),
        _ => {
            // Check for string prefix marker (preserves numeric strings)
            if s.starts_with(STRING_PREFIX) {
                let inner = &s[STRING_PREFIX.len_utf8()..];
                return Ok(json!(inner.replace(SPACE_MARKER, " ")));
            }
            // Try number
            if let Ok(n) = s.parse::<i64>() {
                return Ok(json!(n));
            }
            if let Ok(f) = s.parse::<f64>() {
                return Ok(json!(f));
            }
            // String (restore spaces)
            Ok(json!(s.replace(SPACE_MARKER, " ")))
        }
    }
}

/// Build JSON structure from path-value map
fn build_json_from_paths(
    paths: &std::collections::HashMap<String, serde_json::Value>,
) -> Result<serde_json::Value, SchemaError> {
    use serde_json::json;

    if paths.is_empty() {
        return Ok(json!({}));
    }

    let mut root = json!({});

    for (path, value) in paths {
        insert_at_path(&mut root, path, value.clone())?;
    }

    Ok(root)
}

/// Insert a value at a given path in JSON structure
fn insert_at_path(
    root: &mut serde_json::Value,
    path: &str,
    value: serde_json::Value,
) -> Result<(), SchemaError> {
    use serde_json::Value;

    if path.is_empty() {
        *root = value;
        return Ok(());
    }

    let parts: Vec<&str> = path.split(NEST_SEP).collect();

    // Recursive helper to avoid borrow checker issues
    fn insert_recursive(
        current: &mut Value,
        parts: &[&str],
        value: &Value,
    ) -> Result<(), SchemaError> {
        use serde_json::{Value, json};

        if parts.is_empty() {
            return Ok(());
        }

        let part = parts[0];
        let is_last = parts.len() == 1;

        // Check for object key prefix (marks numeric-looking object keys)
        let is_object_key = part.starts_with(OBJECT_KEY_PREFIX);
        let actual_key = if is_object_key {
            &part[OBJECT_KEY_PREFIX.len_utf8()..]
        } else {
            part
        };

        // Check if this part is an array index (only if not marked as object key)
        if !is_object_key && let Ok(index) = part.parse::<usize>() {
            // Ensure current is an array
            if !current.is_array() {
                *current = json!([]);
            }

            let arr = current.as_array_mut().unwrap();

            // Extend array if needed
            while arr.len() <= index {
                arr.push(Value::Null);
            }

            if is_last {
                arr[index] = value.clone();
            } else {
                // Prepare next container
                let next_part = parts[1];
                if next_part.parse::<usize>().is_ok() {
                    if !arr[index].is_array() {
                        arr[index] = json!([]);
                    }
                } else if !arr[index].is_object() {
                    arr[index] = json!({});
                }
                insert_recursive(&mut arr[index], &parts[1..], value)?;
            }
        } else {
            // Object key - use actual_key (with prefix stripped if present)
            if !current.is_object() {
                *current = json!({});
            }

            if is_last {
                current
                    .as_object_mut()
                    .unwrap()
                    .insert(actual_key.to_string(), value.clone());
            } else {
                // Prepare next container - check if next part is array index (without # prefix)
                let next_part = parts[1];
                let next_is_object_key = next_part.starts_with(OBJECT_KEY_PREFIX);
                let obj = current.as_object_mut().unwrap();

                if !next_is_object_key && next_part.parse::<usize>().is_ok() {
                    obj.entry(actual_key.to_string())
                        .or_insert_with(|| json!([]));
                } else {
                    obj.entry(actual_key.to_string())
                        .or_insert_with(|| json!({}));
                }

                let next = obj.get_mut(actual_key).unwrap();
                insert_recursive(next, &parts[1..], value)?;
            }
        }

        Ok(())
    }

    insert_recursive(root, &parts, &value)
}

/// Serialize IR to ASCII inline stele format (no Unicode symbols)
/// Format: schema;dictionary;row;row;row...
/// Example: id,name,score,active;V1=true,V2=false;1,Alice,95,V1;2,Bob,87,V2
pub fn serialize_ascii(ir: &IntermediateRepresentation) -> Result<String, SchemaError> {
    use std::collections::HashMap;

    let mut output = String::new();

    // Build value dictionary for strings appearing 2+ times
    let mut value_counts: HashMap<String, usize> = HashMap::new();
    let field_count = ir.header.fields.len();

    for row in 0..ir.header.row_count {
        for field_idx in 0..ir.header.fields.len() {
            if ir.is_null(row, field_idx) {
                continue;
            }
            let value_idx = row * field_count + field_idx;
            let value = &ir.values[value_idx];

            if let SchemaValue::String(s) = value {
                *value_counts.entry(s.clone()).or_insert(0) += 1;
            } else if let SchemaValue::Bool(b) = value {
                let b_str = b.to_string();
                *value_counts.entry(b_str).or_insert(0) += 1;
            }
        }
    }

    // Build dictionary for values with count >= 2
    let mut value_dict: HashMap<String, String> = HashMap::new();
    let mut reverse_dict: HashMap<String, String> = HashMap::new();
    let mut sorted_values: Vec<_> = value_counts
        .iter()
        .filter(|(_, count)| **count >= 2)
        .collect();
    sorted_values.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

    for (idx, (value, _)) in sorted_values.iter().enumerate() {
        let token = format!("V{}", idx + 1);
        value_dict.insert((*value).clone(), token.clone());
        reverse_dict.insert(token, (*value).clone());
    }

    // Schema header: field1,field2,field3 (with optional type suffixes)
    for (idx, field) in ir.header.fields.iter().enumerate() {
        if idx > 0 {
            output.push(',');
        }
        // Preserve spaces in field names - commas/semicolons are unambiguous delimiters
        output.push_str(&field.name);

        // Optional type suffix
        let type_suffix = match field.field_type {
            FieldType::I64 | FieldType::U64 => ":i",
            FieldType::String => ":s",
            FieldType::F64 => ":f",
            FieldType::Bool => ":b",
            FieldType::Array(_) => ":a",
            _ => "",
        };
        output.push_str(type_suffix);
    }

    // Value dictionary section (if present)
    if !value_dict.is_empty() {
        output.push(';');
        let mut sorted_dict: Vec<_> = reverse_dict.iter().collect();
        sorted_dict.sort_by_key(|(token, _)| *token);

        for (idx, (token, value)) in sorted_dict.iter().enumerate() {
            if idx > 0 {
                output.push(',');
            }
            output.push_str(token);
            output.push('=');
            // Preserve spaces in dictionary values
            output.push_str(value);
        }
    }

    // Data rows
    for row in 0..ir.header.row_count {
        output.push(';');

        for (field_idx, _field) in ir.header.fields.iter().enumerate() {
            if field_idx > 0 {
                output.push(',');
            }

            if ir.is_null(row, field_idx) {
                // Empty field = null
                continue;
            }

            let value_idx = row * field_count + field_idx;
            let value = &ir.values[value_idx];

            match value {
                SchemaValue::U64(n) => output.push_str(&n.to_string()),
                SchemaValue::I64(n) => output.push_str(&n.to_string()),
                SchemaValue::F64(n) => {
                    if n.fract() == 0.0 && n.abs() < 1e15 {
                        output.push_str(&format!("{:.1}", n));
                    } else {
                        output.push_str(&n.to_string());
                    }
                }
                SchemaValue::String(s) => {
                    if let Some(token) = value_dict.get(s) {
                        output.push_str(token);
                    } else {
                        // Preserve spaces - delimiters are unambiguous
                        output.push_str(s);
                    }
                }
                SchemaValue::Bool(b) => {
                    let b_str = b.to_string();
                    if let Some(token) = value_dict.get(&b_str) {
                        output.push_str(token);
                    } else {
                        output.push_str(&b_str);
                    }
                }
                SchemaValue::Null => {
                    // Already handled by is_null check
                }
                SchemaValue::Array(arr) => {
                    // Arrays serialized as pipe-separated values (preserves spaces in elements)
                    for (i, elem) in arr.iter().enumerate() {
                        if i > 0 {
                            output.push('|');
                        }
                        match elem {
                            SchemaValue::String(s) => output.push_str(s),
                            SchemaValue::I64(n) => output.push_str(&n.to_string()),
                            SchemaValue::U64(n) => output.push_str(&n.to_string()),
                            SchemaValue::F64(n) => output.push_str(&n.to_string()),
                            SchemaValue::Bool(b) => output.push_str(&b.to_string()),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    Ok(output)
}

/// Serialize IR to inline markdown-like format
///
/// Format: `#1 Title;#2 Heading;p Text;-1 Item;-2 Nested;```lang code```;>1 Quote`
///
/// Uses markdown-familiar syntax but inline (semicolon-separated):
/// - `#1`-`#6` for heading levels (not `##`)
/// - `-1`-`-9` for unordered list depth
/// - `+1`-`+9` for ordered list depth
/// - `p` for paragraphs
/// - `` ```lang ``` `` for code blocks
/// - `>1`-`>9` for blockquote depth
/// - `---` for horizontal rule
/// - `[text](url)` for links
/// - `![alt](url)` for images
pub fn serialize_markdown(ir: &IntermediateRepresentation) -> Result<String, SchemaError> {
    let mut output = String::new();
    let field_count = ir.header.fields.len();

    // Expected fields for markdown doc: type, content, meta
    if field_count < 2 {
        return Err(SchemaError::InvalidInput(
            "Markdown IR requires at least type and content fields".to_string(),
        ));
    }

    for row in 0..ir.header.row_count {
        if row > 0 {
            output.push(';');
        }

        // Get type (field 0)
        let type_idx = row * field_count;
        let block_type = match &ir.values[type_idx] {
            SchemaValue::String(s) => s.as_str(),
            _ => continue,
        };

        // Get content (field 1)
        let content_idx = row * field_count + 1;
        let content = if ir.is_null(row, 1) {
            ""
        } else {
            match &ir.values[content_idx] {
                SchemaValue::String(s) => s.as_str(),
                _ => "",
            }
        };

        // Get meta (field 2) if present
        let meta = if field_count > 2 && !ir.is_null(row, 2) {
            let meta_idx = row * field_count + 2;
            match &ir.values[meta_idx] {
                SchemaValue::String(s) => Some(s.as_str()),
                _ => None,
            }
        } else {
            None
        };

        // Convert block type to markdown-like format
        match block_type {
            "h1" => {
                output.push_str("#1 ");
                output.push_str(content);
            }
            "h2" => {
                output.push_str("#2 ");
                output.push_str(content);
            }
            "h3" => {
                output.push_str("#3 ");
                output.push_str(content);
            }
            "h4" => {
                output.push_str("#4 ");
                output.push_str(content);
            }
            "h5" => {
                output.push_str("#5 ");
                output.push_str(content);
            }
            "h6" => {
                output.push_str("#6 ");
                output.push_str(content);
            }
            "p" => {
                output.push_str("p ");
                // Replace newlines with spaces for inline format
                output.push_str(&content.replace('\n', " "));
            }
            "ul" => {
                // Unordered list items
                // Format: semicolons separate same-level items, newlines indicate nesting
                // Example: "Item 1;Item 2\n  Nested;Item 3"
                let mut first = true;
                for line in content.split('\n') {
                    if line.is_empty() {
                        continue;
                    }
                    // Detect nesting level from leading spaces
                    let trimmed = line.trim_start();
                    let indent = line.len() - trimmed.len();
                    let level = (indent / 2) + 1;

                    // Split line by semicolons for multiple items at same level
                    for item in trimmed.split(';') {
                        if item.is_empty() {
                            continue;
                        }
                        if !first {
                            output.push(';');
                        }
                        first = false;
                        output.push('-');
                        output.push_str(&level.to_string());
                        output.push(' ');
                        output.push_str(item.trim());
                    }
                }
            }
            "ol" => {
                // Ordered list items - same logic as ul
                let mut first = true;
                for line in content.split('\n') {
                    if line.is_empty() {
                        continue;
                    }
                    let trimmed = line.trim_start();
                    let indent = line.len() - trimmed.len();
                    let level = (indent / 2) + 1;

                    for item in trimmed.split(';') {
                        if item.is_empty() {
                            continue;
                        }
                        if !first {
                            output.push(';');
                        }
                        first = false;
                        output.push('+');
                        output.push_str(&level.to_string());
                        output.push(' ');
                        output.push_str(item.trim());
                    }
                }
            }
            "code" => {
                output.push_str("```");
                if let Some(lang) = meta {
                    output.push_str(lang);
                }
                output.push(' ');
                // Replace newlines with a marker that won't conflict
                output.push_str(&content.replace('\n', "↵"));
                output.push_str("```");
            }
            "quote" => {
                output.push_str(">1 ");
                output.push_str(&content.replace('\n', "↵"));
            }
            "hr" => {
                output.push_str("---");
            }
            "link" => {
                output.push('[');
                output.push_str(content);
                output.push_str("](");
                if let Some(url) = meta {
                    output.push_str(url);
                }
                output.push(')');
            }
            "image" => {
                output.push_str("![");
                output.push_str(content);
                output.push_str("](");
                if let Some(url) = meta {
                    output.push_str(url);
                }
                output.push(')');
            }
            "table" => {
                // Keep table as-is for now, it's already compact
                output.push_str("T ");
                output.push_str(content);
                if let Some(dims) = meta {
                    output.push(' ');
                    output.push_str(dims);
                }
            }
            _ => {
                // Unknown type - output as-is
                output.push_str(block_type);
                output.push(' ');
                output.push_str(content);
            }
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_roundtrip() {
        // Superscript format (spec 1.7+), non-tokenized for clarity
        let fiche = "@users┃idⁱ┃nameˢ┃activeᵇ
◉1┃alice┃true
◉2┃bob┃false";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.row_count, 2);
        assert_eq!(ir.header.fields.len(), 3);
        assert_eq!(ir.header.root_key, Some("users".to_string()));

        // Use readable (non-tokenized) for roundtrip
        let output = serialize_readable(&ir, false).unwrap();
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_tokenized_roundtrip() {
        // Non-tokenized input
        let fiche = "@users┃idⁱ┃nameˢ┃activeᵇ
◉1┃alice┃true
◉2┃bob┃false";

        let ir = parse(fiche).unwrap();

        // Tokenized output should have token map
        let tokenized = serialize(&ir, false).unwrap();
        assert!(tokenized.starts_with("@ᚠ=id,ᚡ=name,ᚢ=active\n"));
        assert!(tokenized.contains("┃ᚠⁱ┃ᚡˢ┃ᚢᵇ"));

        // Parse tokenized output back
        let ir2 = parse(&tokenized).unwrap();
        assert_eq!(ir2.header.row_count, 2);
        assert_eq!(ir2.header.fields.len(), 3);

        // Field names should be restored
        assert_eq!(ir2.header.fields[0].name, "id");
        assert_eq!(ir2.header.fields[1].name, "name");
        assert_eq!(ir2.header.fields[2].name, "active");
    }

    #[test]
    fn test_legacy_type_format_parsing() {
        // Legacy format (pre 1.7) should still parse
        let fiche = "@users┃id:int┃name:str┃active:bool
◉1┃alice┃true
◉2┃bob┃false";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.row_count, 2);
        assert_eq!(ir.header.fields.len(), 3);

        // Output uses superscript format (readable mode)
        let output = serialize_readable(&ir, false).unwrap();
        assert!(output.contains("idⁱ"));
        assert!(output.contains("nameˢ"));
        assert!(output.contains("activeᵇ"));
    }

    #[test]
    fn test_arrays_legacy_syntax() {
        // Test backward compatibility with old str[] syntax
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

        // Output uses superscript + ⟦⟧ syntax (readable mode)
        let output = serialize_readable(&ir, false).unwrap();
        assert!(output.contains("tagsˢ⟦⟧"));
    }

    #[test]
    fn test_arrays_new_bracket_syntax() {
        // Test new superscript + ⟦⟧ syntax
        let fiche = "@users┃idⁱ┃tagsˢ⟦⟧
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

        // Roundtrip with superscript format (readable mode)
        let output = serialize_readable(&ir, false).unwrap();
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_nulls() {
        let fiche = "@records┃idⁱ┃scoreᶠ┃notesˢ
◉1┃95.5┃∅
◉2┃∅┃pending";

        let ir = parse(fiche).unwrap();
        assert!(ir.is_null(0, 2)); // notes is null for row 0
        assert!(ir.is_null(1, 1)); // score is null for row 1

        let output = serialize_readable(&ir, false).unwrap();
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_embedded_json() {
        let fiche = r#"@logs┃levelˢ┃msgˢ
◉error┃Failed▓to▓parse▓{"key":▓"value"}"#;

        let ir = parse(fiche).unwrap();

        if let Some(SchemaValue::String(msg)) = ir.get_value(0, 1) {
            assert_eq!(msg, r#"Failed to parse {"key": "value"}"#);
        } else {
            panic!("Expected string");
        }

        let output = serialize_readable(&ir, false).unwrap();
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_no_root_key() {
        let fiche = "@┃idⁱ┃nameˢ
◉1┃alice";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.root_key, None);
    }

    #[test]
    fn test_type_parsing() {
        // Legacy format
        assert!(matches!(parse_type_str("int"), Ok(FieldType::I64)));
        assert!(matches!(parse_type_str("str"), Ok(FieldType::String)));
        assert!(matches!(parse_type_str("float"), Ok(FieldType::F64)));
        assert!(matches!(parse_type_str("bool"), Ok(FieldType::Bool)));

        // Superscript format
        assert!(matches!(parse_type_str("ⁱ"), Ok(FieldType::I64)));
        assert!(matches!(parse_type_str("ˢ"), Ok(FieldType::String)));
        assert!(matches!(parse_type_str("ᶠ"), Ok(FieldType::F64)));
        assert!(matches!(parse_type_str("ᵇ"), Ok(FieldType::Bool)));

        // Test legacy [] syntax
        assert!(matches!(
            parse_type_str("str[]"),
            Ok(FieldType::Array(box_inner)) if *box_inner == FieldType::String
        ));
        // Test new ⟦⟧ syntax with legacy type
        assert!(matches!(
            parse_type_str("str⟦⟧"),
            Ok(FieldType::Array(box_inner)) if *box_inner == FieldType::String
        ));
        // Test superscript + ⟦⟧ syntax
        assert!(matches!(
            parse_type_str("ˢ⟦⟧"),
            Ok(FieldType::Array(box_inner)) if *box_inner == FieldType::String
        ));
    }

    #[test]
    fn test_nested_arrays() {
        // Inline primitive arrays use ◈ separator
        let fiche = "@people┃nameˢ┃heightˢ┃filmsˢ⟦⟧┃vehiclesˢ⟦⟧
◉Luke┃172┃film/1◈film/2┃∅
◉Leia┃150┃film/1┃vehicle/30";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.row_count, 2);
        assert_eq!(ir.header.fields.len(), 4);

        // Check Luke's name (scalar string)
        if let Some(SchemaValue::String(name)) = ir.get_value(0, 0) {
            assert_eq!(name, "Luke");
        } else {
            panic!("Expected string");
        }

        // Check Luke's height
        if let Some(SchemaValue::String(height)) = ir.get_value(0, 1) {
            assert_eq!(height, "172");
        } else {
            panic!("Expected string");
        }

        // Check Luke's films (array)
        if let Some(SchemaValue::Array(films)) = ir.get_value(0, 2) {
            assert_eq!(films.len(), 2);
            if let SchemaValue::String(film) = &films[0] {
                assert_eq!(film, "film/1");
            } else {
                panic!("Expected string");
            }
        } else {
            panic!("Expected array");
        }

        // Check Luke's empty vehicles
        if let Some(SchemaValue::Array(vehicles)) = ir.get_value(0, 3) {
            assert_eq!(vehicles.len(), 0);
        } else {
            panic!("Expected array");
        }

        // Check Leia's vehicles
        if let Some(SchemaValue::Array(vehicles)) = ir.get_value(1, 3) {
            assert_eq!(vehicles.len(), 1);
            if let SchemaValue::String(vehicle) = &vehicles[0] {
                assert_eq!(vehicle, "vehicle/30");
            } else {
                panic!("Expected string");
            }
        } else {
            panic!("Expected array");
        }

        let output = serialize_readable(&ir, false).unwrap();
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_space_preservation() {
        let fiche = "@people┃nameˢ┃homeˢ
◉Luke▓Skywalker┃Tatooine▓Desert▓Planet
◉Leia▓Organa┃Alderaan";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.row_count, 2);

        // Check decoded values have spaces
        if let Some(SchemaValue::String(name)) = ir.get_value(0, 0) {
            assert_eq!(name, "Luke Skywalker");
        } else {
            panic!("Expected string");
        }

        if let Some(SchemaValue::String(home)) = ir.get_value(0, 1) {
            assert_eq!(home, "Tatooine Desert Planet");
        } else {
            panic!("Expected string");
        }

        // Check re-encoding produces minified spaces (readable mode)
        let output = serialize_readable(&ir, false).unwrap();
        assert!(output.contains("Luke▓Skywalker"));
        assert!(output.contains("Tatooine▓Desert▓Planet"));
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_minified_output() {
        // Test that minified output uses ▓ for line breaks and roundtrips correctly
        let fiche_normal = "@users┃idⁱ┃nameˢ
◉1┃alice
◉2┃bob";

        let ir = parse(fiche_normal).unwrap();

        // Serialize minified (tokenized) - check structure
        let minified = serialize_minified(&ir).unwrap();
        assert!(!minified.contains('\n'), "Minified should have no newlines");
        assert!(
            minified.contains('▓'),
            "Minified should use ▓ as line separator"
        );

        // Parse minified back - should restore field names
        let ir2 = parse(&minified).unwrap();
        assert_eq!(ir2.header.row_count, 2);
        assert_eq!(ir2.header.fields[0].name, "id");
        assert_eq!(ir2.header.fields[1].name, "name");

        // Values should match
        if let Some(SchemaValue::String(name)) = ir2.get_value(0, 1) {
            assert_eq!(name, "alice");
        } else {
            panic!("Expected string");
        }
        if let Some(SchemaValue::String(name)) = ir2.get_value(1, 1) {
            assert_eq!(name, "bob");
        } else {
            panic!("Expected string");
        }
    }

    #[test]
    fn test_metadata_annotation() {
        let fiche = "@students[class=Year▓1,school_name=Springfield▓High]┃idˢ
◉A1
◉B2";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.root_key, Some("students".to_string()));
        assert_eq!(ir.header.row_count, 2);

        // Check metadata
        assert!(ir.header.metadata.is_some());
        let metadata = ir.header.metadata.as_ref().unwrap();
        assert_eq!(
            metadata.get("school_name"),
            Some(&"Springfield High".to_string())
        );
        assert_eq!(metadata.get("class"), Some(&"Year 1".to_string()));

        // Check roundtrip (readable mode)
        let output = serialize_readable(&ir, false).unwrap();
        assert!(output.contains("[class=Year▓1,school_name=Springfield▓High]"));
        assert_eq!(output, fiche);
    }

    #[test]
    fn test_metadata_minified() {
        let fiche = "@students[class=Year▓1,school_name=Springfield▓High]┃idˢ
◉A1
◉B2";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.row_count, 2);

        // Check metadata
        assert!(ir.header.metadata.is_some());
        let metadata = ir.header.metadata.as_ref().unwrap();
        assert_eq!(
            metadata.get("school_name"),
            Some(&"Springfield High".to_string())
        );
        assert_eq!(metadata.get("class"), Some(&"Year 1".to_string()));

        // Tokenized minified output roundtrips back to same structure
        let tokenized = serialize_minified(&ir).unwrap();
        let ir2 = parse(&tokenized).unwrap();
        assert_eq!(ir2.header.fields[0].name, "id");
        assert_eq!(ir2.header.row_count, 2);
    }

    #[test]
    fn test_value_dictionary() {
        // Service logs with repeated levels and service names
        let fiche = "@logs┃levelˢ┃messageˢ┃serviceˢ
◉info┃Request▓received┃api
◉debug┃Parsing▓payload┃api
◉info┃Auth▓validated┃api
◉error┃Connection▓timeout┃db
◉info┃Response▓sent┃api
◉error┃Query▓failed┃db";

        let ir = parse(fiche).unwrap();
        assert_eq!(ir.header.row_count, 6);

        // Serialize with value dictionary (default for serialize)
        let tokenized = serialize(&ir, false).unwrap();

        // Should have field dictionary
        assert!(tokenized.starts_with("@ᚠ=level,ᚡ=message,ᚢ=service\n"));

        // Should have value dictionary (info, api, error, db appear 2+ times)
        assert!(tokenized.contains("@𓀀="));

        // Parse back and verify roundtrip
        let ir2 = parse(&tokenized).unwrap();
        assert_eq!(ir2.header.row_count, 6);
        assert_eq!(ir2.header.fields.len(), 3);

        // Check field names restored
        assert_eq!(ir2.header.fields[0].name, "level");
        assert_eq!(ir2.header.fields[1].name, "message");
        assert_eq!(ir2.header.fields[2].name, "service");

        // Check values decoded correctly
        if let Some(SchemaValue::String(level)) = ir2.get_value(0, 0) {
            assert_eq!(level, "info");
        } else {
            panic!("Expected string value");
        }

        if let Some(SchemaValue::String(service)) = ir2.get_value(0, 2) {
            assert_eq!(service, "api");
        } else {
            panic!("Expected string value");
        }

        // Verify error level in row 3
        if let Some(SchemaValue::String(level)) = ir2.get_value(3, 0) {
            assert_eq!(level, "error");
        } else {
            panic!("Expected string value");
        }
    }

    #[test]
    fn test_value_dictionary_no_duplicates() {
        // All unique values - should not generate value dictionary
        let fiche = "@data┃idˢ┃nameˢ
◉1┃alice
◉2┃bob
◉3┃carol";

        let ir = parse(fiche).unwrap();
        let tokenized = serialize(&ir, false).unwrap();

        // Should have field dictionary
        assert!(tokenized.starts_with("@ᚠ=id,ᚡ=name\n"));

        // Should NOT have value dictionary (no repeated values)
        let lines: Vec<&str> = tokenized.lines().collect();
        assert_eq!(lines.len(), 5); // field dict, schema, 3 data rows
        assert!(!tokenized.contains("𓀀")); // No hieroglyphs
    }

    #[test]
    fn test_path_mode_roundtrip_simple() {
        // Simple nested object
        let json = r#"{"a":1,"b":{"c":"hello","d":true}}"#;
        let fiche = serialize_path_mode(json).unwrap();
        let result = parse_path_mode(&fiche).unwrap();

        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_path_mode_roundtrip_arrays() {
        // Arrays with indices
        let json = r#"{"users":[{"name":"alice"},{"name":"bob"}]}"#;
        let fiche = serialize_path_mode(json).unwrap();
        let result = parse_path_mode(&fiche).unwrap();

        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_path_mode_roundtrip_nulls_bools() {
        // Null and boolean handling
        let json = r#"{"active":true,"deleted":false,"data":null}"#;
        let fiche = serialize_path_mode(json).unwrap();
        let result = parse_path_mode(&fiche).unwrap();

        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_path_mode_roundtrip_empty_containers() {
        // Empty arrays and objects
        let json = r#"{"items":[],"meta":{}}"#;
        let fiche = serialize_path_mode(json).unwrap();
        let result = parse_path_mode(&fiche).unwrap();

        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_path_mode_roundtrip_deep_nesting() {
        // Deep nesting (the use case path mode was built for)
        let json = r#"{"a":{"b":{"c":{"d":{"e":1}}}}}"#;
        let fiche = serialize_path_mode(json).unwrap();
        let result = parse_path_mode(&fiche).unwrap();

        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_path_mode_roundtrip_with_tokenization() {
        // Verify path and value tokenization roundtrips
        // "users", "name", "role" should be tokenized (runic) - appear 2+ times in paths
        // "admin" should be tokenized (hieroglyph) - appears 2x
        let json = r#"{"users":[{"name":"alice","role":"admin"},{"name":"bob","role":"admin"}]}"#;
        let fiche = serialize_path_mode(json).unwrap();

        // Verify path tokenization occurred (runic characters should be present)
        assert!(
            fiche.chars().any(tokens::is_token),
            "Expected runic tokens for path segments"
        );

        // Verify value tokenization occurred (hieroglyph for "admin")
        assert!(
            fiche.chars().any(value_tokens::is_token),
            "Expected hieroglyph token for repeated value"
        );

        let result = parse_path_mode(&fiche).unwrap();

        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_ascii_mode_simple() {
        use crate::encoders::algorithms::schema::parsers::{InputParser, JsonParser};

        let json = r#"{"users":[{"id":1,"name":"Alice","score":95,"active":true},{"id":2,"name":"Bob","score":87,"active":false}]}"#;
        let ir = JsonParser::parse(json).unwrap();
        let ascii = serialize_ascii(&ir).unwrap();

        // Verify format: schema;row;row
        assert!(ascii.contains(';'));
        assert!(ascii.contains(','));

        // Verify ASCII only
        assert!(ascii.is_ascii());

        // Verify no Unicode symbols
        assert!(!ascii.contains('◉'));
        assert!(!ascii.contains('┃'));
        assert!(!ascii.contains('▓'));
    }

    #[test]
    fn test_ascii_mode_with_dictionary() {
        use crate::encoders::algorithms::schema::parsers::{InputParser, JsonParser};

        // Repeated values should trigger dictionary
        let json = r#"{"logs":[{"level":"info","msg":"start"},{"level":"error","msg":"fail"},{"level":"info","msg":"retry"}]}"#;
        let ir = JsonParser::parse(json).unwrap();
        let ascii = serialize_ascii(&ir).unwrap();

        // Verify dictionary section present
        assert!(ascii.contains("V1="));

        // "info" appears twice, should be in dictionary
        assert!(ascii.contains("info") || ascii.contains("V1"));
    }

    #[test]
    fn test_ascii_mode_null_handling() {
        use crate::encoders::algorithms::schema::parsers::{InputParser, JsonParser};

        let json = r#"{"data":[{"a":1,"b":null},{"a":2,"b":"value"}]}"#;
        let ir = JsonParser::parse(json).unwrap();
        let ascii = serialize_ascii(&ir).unwrap();

        // Verify empty field for null
        // Format should be: a:i,b:s;1,;2,value
        let rows: Vec<&str> = ascii.split(';').collect();
        assert!(rows.len() >= 2);

        // Second row (index 1) should have empty field
        assert!(rows[1].contains("1,"));
    }

    #[test]
    fn test_ascii_mode_space_preservation() {
        use crate::encoders::algorithms::schema::parsers::{InputParser, JsonParser};

        let json = r#"{"people":[{"name":"Alice Smith","title":"Senior Engineer"}]}"#;
        let ir = JsonParser::parse(json).unwrap();
        let ascii = serialize_ascii(&ir).unwrap();

        // Spaces should be preserved (not escaped) for better tokenization
        assert!(ascii.contains("Alice Smith"));
        assert!(ascii.contains("Senior Engineer"));
    }

    #[test]
    fn test_path_mode_numeric_object_keys_roundtrip() {
        // Issue #143: Sparse objects with numeric keys should roundtrip correctly
        let json = r#"{"values":{"0":"a","5":"b","10":"c"}}"#;
        let fiche = serialize_path_mode(json).unwrap();

        // Should contain # prefix for numeric object keys
        assert!(fiche.contains("#0"), "Expected #0 marker for object key");
        assert!(fiche.contains("#5"), "Expected #5 marker for object key");
        assert!(fiche.contains("#10"), "Expected #10 marker for object key");

        let result = parse_path_mode(&fiche).unwrap();

        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            original, parsed,
            "Sparse object keys should roundtrip correctly"
        );
    }

    #[test]
    fn test_path_mode_string_numbers_roundtrip() {
        // Issue #143: String numbers should not be coerced to numeric types
        let json = r#"{"id":"1579231263","phone":"5551234567"}"#;
        let fiche = serialize_path_mode(json).unwrap();

        // Should contain " prefix for string values that look like numbers
        assert!(fiche.contains('"'), "Expected \" marker for string numbers");

        let result = parse_path_mode(&fiche).unwrap();

        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            original, parsed,
            "String numbers should roundtrip correctly"
        );
    }

    #[test]
    fn test_path_mode_mixed_array_and_object() {
        // Verify arrays and objects with numeric keys coexist correctly
        let json = r#"{"items":[{"0":"first","1":"second"}],"map":{"0":"zero","1":"one"}}"#;
        let fiche = serialize_path_mode(json).unwrap();

        let result = parse_path_mode(&fiche).unwrap();

        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            original, parsed,
            "Mixed arrays and numeric-keyed objects should roundtrip"
        );
    }
}
