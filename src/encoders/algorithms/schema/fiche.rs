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

// Fiche delimiters
pub const ROW_START: char = '◉'; // U+25C9 fisheye
pub const FIELD_SEP: char = '┃'; // U+2503 heavy pipe
pub const ARRAY_SEP: char = '◈'; // U+25C8 diamond in diamond
pub const NULL_VALUE: &str = "∅"; // U+2205 empty set
pub const SPACE_MARKER: char = '▓'; // U+2593 Dark Shade
pub const NEST_SEP: char = '჻'; // U+10FB Georgian paragraph separator

// Type names in fiche schema (legacy)
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

/// Field name tokenization alphabet (spec 1.5+)
/// Priority: Runic (89 chars) → Hieroglyphs (1072) → Cuneiform (1024)
/// Ancient scripts avoid ASCII/digit collision and regex interference
pub mod tokens {
    /// Runic alphabet (U+16A0 – U+16F8, 89 characters, BMP)
    /// Primary tokenization alphabet - most compact, BMP-only
    pub const RUNIC: &[char] = &[
        'ᚠ', 'ᚡ', 'ᚢ', 'ᚣ', 'ᚤ', 'ᚥ', 'ᚦ', 'ᚧ', 'ᚨ', 'ᚩ', 'ᚪ', 'ᚫ', 'ᚬ', 'ᚭ', 'ᚮ', 'ᚯ',
        'ᚰ', 'ᚱ', 'ᚲ', 'ᚳ', 'ᚴ', 'ᚵ', 'ᚶ', 'ᚷ', 'ᚸ', 'ᚹ', 'ᚺ', 'ᚻ', 'ᚼ', 'ᚽ', 'ᚾ', 'ᚿ',
        'ᛀ', 'ᛁ', 'ᛂ', 'ᛃ', 'ᛄ', 'ᛅ', 'ᛆ', 'ᛇ', 'ᛈ', 'ᛉ', 'ᛊ', 'ᛋ', 'ᛌ', 'ᛍ', 'ᛎ', 'ᛏ',
        'ᛐ', 'ᛑ', 'ᛒ', 'ᛓ', 'ᛔ', 'ᛕ', 'ᛖ', 'ᛗ', 'ᛘ', 'ᛙ', 'ᛚ', 'ᛛ', 'ᛜ', 'ᛝ', 'ᛞ', 'ᛟ',
        'ᛠ', 'ᛡ', 'ᛢ', 'ᛣ', 'ᛤ', 'ᛥ', 'ᛦ', 'ᛧ', 'ᛨ', 'ᛩ', 'ᛪ', '᛫', '᛬', '᛭', 'ᛮ', 'ᛯ',
        'ᛰ', 'ᛱ', 'ᛲ', 'ᛳ', 'ᛴ', 'ᛵ', 'ᛶ', 'ᛷ', 'ᛸ',
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
    pub fn token_index(c: char) -> Option<usize> {
        RUNIC.iter().position(|&t| t == c)
    }
}

/// Serialize IR to fiche format (with tokenization)
pub fn serialize(ir: &IntermediateRepresentation) -> Result<String, SchemaError> {
    serialize_full_options(ir, false, true)
}

/// Serialize IR to fiche format without tokenization (human-readable field names)
pub fn serialize_readable(ir: &IntermediateRepresentation) -> Result<String, SchemaError> {
    serialize_full_options(ir, false, false)
}

pub fn serialize_minified(ir: &IntermediateRepresentation) -> Result<String, SchemaError> {
    serialize_full_options(ir, true, true)
}

/// Serialize with minify and tokenization options
fn serialize_full_options(
    ir: &IntermediateRepresentation,
    minify: bool,
    tokenize: bool,
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

    // Token map header: @ᚠ=field1,ᚡ=field2,...
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
        output.push_str(&field_type_to_str(&field.field_type));
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
        // Superscript format: no colon separator, type marker suffixes the name
        output.push_str(&field_type_to_str(&field.field_type));
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

/// Parse fiche format to IR
/// Supports both tokenized and non-tokenized formats
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

    // Parse token map if present (tokenized format)
    // Format: @ᚠ=field1,ᚡ=field2,...\n@root┃...
    // Or minified: @ᚠ=field1,ᚡ=field2,...▓@root┃...
    let mut token_map: std::collections::HashMap<char, String> = std::collections::HashMap::new();
    let schema_part = schema_part.trim();

    // Check if first line is a token map (starts with @{runic}= pattern)
    let (effective_schema, _token_map_used) = {
        let lines: Vec<&str> = schema_part
            .split(|c| c == '\n' || c == SPACE_MARKER)
            .filter(|s| !s.is_empty())
            .collect();

        if lines.len() >= 2 {
            let first_line = lines[0];
            // Token map: starts with @ followed by a runic token and =
            // This distinguishes it from metadata annotations like @root[key=value]
            let is_token_map = if first_line.starts_with('@') && first_line.len() > 1 {
                let after_at = &first_line[1..];
                // Check if first char is a runic token followed by =
                let first_char = after_at.chars().next();
                first_char.is_some_and(|c| tokens::is_token(c)) && after_at.contains('=')
            } else {
                false
            };

            if is_token_map {
                // Parse token map: @ᚠ=field1,ᚡ=field2,...
                let map_content = &first_line[1..]; // Remove @
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
                // Use remaining lines as schema
                (lines[1..].join(&SPACE_MARKER.to_string()), true)
            } else {
                (schema_part.to_string(), false)
            }
        } else {
            (schema_part.to_string(), false)
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

/// Convert FieldType to fiche type string (superscript format)
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

/// Parse fiche type string to FieldType
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
    if chars.len() == 1 {
        if let Some(resolved_name) = token_map.get(&chars[0]) {
            return Ok((resolved_name.clone(), field_type));
        }
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
    let (base, is_array) = if s.ends_with("⟦⟧") {
        (&s[..s.len() - "⟦⟧".len()], true)
    } else if s.ends_with("[]") {
        (&s[..s.len() - 2], true)
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
        let output = serialize_readable(&ir).unwrap();
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
        let tokenized = serialize(&ir).unwrap();
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
        let output = serialize_readable(&ir).unwrap();
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
        let output = serialize_readable(&ir).unwrap();
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
        let output = serialize_readable(&ir).unwrap();
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

        let output = serialize_readable(&ir).unwrap();
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

        let output = serialize_readable(&ir).unwrap();
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

        let output = serialize_readable(&ir).unwrap();
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
        let output = serialize_readable(&ir).unwrap();
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
        let output = serialize_readable(&ir).unwrap();
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
}
