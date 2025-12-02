use std::fmt;

/// Field types supported in schema encoding
///
/// Each field in the schema has a declared type. These types map to JSON types
/// and are encoded with 4-bit type tags in the binary format.
///
/// # Type Mapping
///
/// * `U64` - JSON numbers (positive integers)
/// * `I64` - JSON numbers (negative integers)
/// * `F64` - JSON numbers (floats)
/// * `String` - JSON strings
/// * `Bool` - JSON booleans
/// * `Null` - JSON null
/// * `Array(T)` - JSON arrays (homogeneous element type)
/// * `Any` - Mixed-type values (reserved for future use)
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    U64,
    I64,
    F64,
    String,
    Bool,
    Null,
    Array(Box<FieldType>), // Homogeneous arrays
    Any,                   // For mixed-type arrays when typed_values flag set
}

impl FieldType {
    /// Get the 4-bit type tag for binary encoding
    pub fn type_tag(&self) -> u8 {
        match self {
            FieldType::U64 => 0,
            FieldType::I64 => 1,
            FieldType::F64 => 2,
            FieldType::String => 3,
            FieldType::Bool => 4,
            FieldType::Null => 5,
            FieldType::Array(_) => 6,
            FieldType::Any => 7,
        }
    }

    /// Get user-friendly display name for error messages
    pub fn display_name(&self) -> String {
        match self {
            FieldType::U64 => "unsigned integer".to_string(),
            FieldType::I64 => "signed integer".to_string(),
            FieldType::F64 => "floating-point number".to_string(),
            FieldType::String => "string".to_string(),
            FieldType::Bool => "boolean".to_string(),
            FieldType::Null => "null".to_string(),
            FieldType::Array(element_type) => {
                format!("array of {}", element_type.display_name())
            }
            FieldType::Any => "any type".to_string(),
        }
    }

    /// Construct a FieldType from a 4-bit type tag
    pub fn from_type_tag(
        tag: u8,
        element_type: Option<Box<FieldType>>,
    ) -> Result<Self, SchemaError> {
        match tag {
            0 => Ok(FieldType::U64),
            1 => Ok(FieldType::I64),
            2 => Ok(FieldType::F64),
            3 => Ok(FieldType::String),
            4 => Ok(FieldType::Bool),
            5 => Ok(FieldType::Null),
            6 => {
                if let Some(et) = element_type {
                    Ok(FieldType::Array(et))
                } else {
                    Err(SchemaError::InvalidTypeTag {
                        tag,
                        context: Some("array type requires element type".to_string()),
                    })
                }
            }
            7 => Ok(FieldType::Any),
            _ => Err(SchemaError::InvalidTypeTag { tag, context: None }),
        }
    }
}

/// Header flags (bit positions)
#[allow(dead_code)]
pub const FLAG_TYPED_VALUES: u8 = 0b0000_0001; // Per-value type tags
pub const FLAG_HAS_NULLS: u8 = 0b0000_0010; // Null bitmap present
pub const FLAG_HAS_ROOT_KEY: u8 = 0b0000_0100; // Root key in header

/// Schema header containing metadata about the encoded data
///
/// The header is self-describing and stores:
/// * Flags indicating optional features (nulls, root key, etc.)
/// * Optional root key for single-array JSON structures
/// * Row and field counts
/// * Field definitions (names and types)
/// * Optional null bitmap for tracking null values
///
/// # Binary Format
///
/// ```text
/// [flags: u8]
/// [root_key?: varint_string]  // if FLAG_HAS_ROOT_KEY
/// [row_count: varint]
/// [field_count: varint]
/// [field_types: 4-bit packed]
/// [field_names: varint_strings]
/// [null_bitmap?: bytes]        // if FLAG_HAS_NULLS
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaHeader {
    pub flags: u8,
    pub root_key: Option<String>,
    pub row_count: usize,
    pub fields: Vec<FieldDef>,
    pub null_bitmap: Option<Vec<u8>>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

impl SchemaHeader {
    /// Create a new header
    pub fn new(row_count: usize, fields: Vec<FieldDef>) -> Self {
        Self {
            flags: 0,
            root_key: None,
            row_count,
            fields,
            null_bitmap: None,
            metadata: None,
        }
    }

    /// Set a flag bit
    pub fn set_flag(&mut self, flag: u8) {
        self.flags |= flag;
    }

    /// Check if a flag bit is set
    pub fn has_flag(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    /// Calculate total value count (row_count * field_count)
    pub fn total_value_count(&self) -> usize {
        self.row_count * self.fields.len()
    }
}

/// Field definition with name and type
///
/// Each field in the schema has a name and declared type. For nested objects,
/// field names use dotted notation (e.g., `"user.profile.name"`).
///
/// # Examples
///
/// ```ignore
/// use base_d::encoders::algorithms::schema::types::{FieldDef, FieldType};
///
/// // Simple field
/// let id_field = FieldDef::new("id", FieldType::U64);
///
/// // Nested field (flattened)
/// let name_field = FieldDef::new("user.profile.name", FieldType::String);
///
/// // Array field
/// let tags_field = FieldDef::new("tags", FieldType::Array(Box::new(FieldType::String)));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: String, // Flattened dotted key: "user.profile.avatar"
    pub field_type: FieldType,
}

impl FieldDef {
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
        }
    }
}

/// The format-agnostic intermediate representation
///
/// This is the bridge between input formats (JSON) and the binary encoding.
/// Data is stored in column-oriented, row-major order.
///
/// # Structure
///
/// * **Header**: Schema metadata (field names, types, counts)
/// * **Values**: Flat array of values in row-major order
///
/// # Row-Major Layout
///
/// For 2 rows × 3 fields, values are stored as:
/// ```text
/// [row0_field0, row0_field1, row0_field2, row1_field0, row1_field1, row1_field2]
/// ```
///
/// # Examples
///
/// ```ignore
/// use base_d::{IntermediateRepresentation, SchemaHeader, SchemaValue, FieldDef, FieldType};
///
/// let fields = vec![
///     FieldDef::new("id", FieldType::U64),
///     FieldDef::new("name", FieldType::String),
/// ];
/// let header = SchemaHeader::new(2, fields);
///
/// let values = vec![
///     SchemaValue::U64(1),
///     SchemaValue::String("alice".to_string()),
///     SchemaValue::U64(2),
///     SchemaValue::String("bob".to_string()),
/// ];
///
/// let ir = IntermediateRepresentation::new(header, values)?;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct IntermediateRepresentation {
    pub header: SchemaHeader,
    pub values: Vec<SchemaValue>, // Row-major: field1, field2, field1, field2...
}

impl IntermediateRepresentation {
    /// Create a new IR
    pub fn new(header: SchemaHeader, values: Vec<SchemaValue>) -> Result<Self, SchemaError> {
        // Validate value count matches header
        let expected_count = header.total_value_count();
        if values.len() != expected_count {
            return Err(SchemaError::ValueCountMismatch {
                expected: expected_count,
                actual: values.len(),
            });
        }

        Ok(Self { header, values })
    }

    /// Get value at row and field index
    pub fn get_value(&self, row: usize, field: usize) -> Option<&SchemaValue> {
        if row >= self.header.row_count || field >= self.header.fields.len() {
            return None;
        }
        let idx = row * self.header.fields.len() + field;
        self.values.get(idx)
    }

    /// Check if a value is null according to the null bitmap
    pub fn is_null(&self, row: usize, field: usize) -> bool {
        if let Some(bitmap) = &self.header.null_bitmap {
            let idx = row * self.header.fields.len() + field;
            let byte_idx = idx / 8;
            let bit_idx = idx % 8;
            if byte_idx < bitmap.len() {
                return (bitmap[byte_idx] >> bit_idx) & 1 == 1;
            }
        }
        false
    }
}

/// Schema value representing a single data element
///
/// Values map directly to JSON types and are encoded using type-specific
/// binary representations (varint for integers, IEEE 754 for floats, etc.).
///
/// # Binary Encoding
///
/// * `U64` - Varint (variable-length)
/// * `I64` - Zigzag varint (converts negatives to small positives)
/// * `F64` - 8 bytes IEEE 754 little-endian
/// * `String` - Varint length + UTF-8 bytes
/// * `Bool` - Single bit (packed 8 per byte)
/// * `Null` - Zero bytes (tracked in null bitmap)
/// * `Array` - Varint count + elements
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaValue {
    U64(u64),
    I64(i64),
    F64(f64),
    String(String),
    Bool(bool),
    Null,
    Array(Vec<SchemaValue>),
}

impl SchemaValue {
    /// Get the type tag for this value
    #[allow(dead_code)]
    pub fn type_tag(&self) -> u8 {
        match self {
            SchemaValue::U64(_) => 0,
            SchemaValue::I64(_) => 1,
            SchemaValue::F64(_) => 2,
            SchemaValue::String(_) => 3,
            SchemaValue::Bool(_) => 4,
            SchemaValue::Null => 5,
            SchemaValue::Array(_) => 6,
        }
    }

    /// Check if value matches field type
    #[allow(dead_code)]
    pub fn matches_type(&self, field_type: &FieldType) -> bool {
        matches!(
            (self, field_type),
            (SchemaValue::U64(_), FieldType::U64)
                | (SchemaValue::I64(_), FieldType::I64)
                | (SchemaValue::F64(_), FieldType::F64)
                | (SchemaValue::String(_), FieldType::String)
                | (SchemaValue::Bool(_), FieldType::Bool)
                | (SchemaValue::Null, FieldType::Null)
                | (SchemaValue::Array(_), FieldType::Array(_))
                | (_, FieldType::Any)
        )
    }
}

/// Errors that can occur during schema encoding/decoding
///
/// This error type covers all failure modes in the schema encoding pipeline:
/// parsing, binary packing/unpacking, compression, and framing.
///
/// # Examples
///
/// ```ignore
/// use base_d::{decode_schema, SchemaError};
///
/// let result = decode_schema("invalid", false);
/// match result {
///     Err(SchemaError::InvalidFrame(msg)) => {
///         println!("Frame error: {}", msg);
///     }
///     Err(SchemaError::InvalidCharacter(msg)) => {
///         println!("Character error: {}", msg);
///     }
///     _ => {}
/// }
/// ```
#[derive(Debug, PartialEq)]
pub enum SchemaError {
    /// Invalid type tag encountered
    InvalidTypeTag { tag: u8, context: Option<String> },
    /// Value count mismatch
    ValueCountMismatch { expected: usize, actual: usize },
    /// Unexpected end of data
    UnexpectedEndOfData { context: String, position: usize },
    /// Invalid varint encoding
    InvalidVarint { context: String, position: usize },
    /// Invalid UTF-8 string
    InvalidUtf8 {
        context: String,
        error: std::string::FromUtf8Error,
    },
    /// Type mismatch between value and field type
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
        row: Option<usize>,
    },
    /// Invalid null bitmap size
    InvalidNullBitmap {
        expected_bytes: usize,
        actual_bytes: usize,
    },
    /// Invalid input format
    InvalidInput(String),
    /// Invalid frame delimiters
    InvalidFrame(String),
    /// Invalid character in encoded data
    InvalidCharacter(String),
    /// Compression error
    Compression(String),
    /// Decompression error
    Decompression(String),
    /// Invalid compression algorithm byte
    InvalidCompressionAlgorithm(u8),
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaError::InvalidTypeTag { tag, context } => {
                write!(
                    f,
                    "Invalid type tag {} at byte offset{}. Valid type tags are 0-7.",
                    tag,
                    context
                        .as_ref()
                        .map(|c| format!(" ({})", c))
                        .unwrap_or_default()
                )
            }
            SchemaError::ValueCountMismatch { expected, actual } => {
                write!(
                    f,
                    "Value count mismatch: expected {} values but found {}.\n\
                     This indicates corrupted or truncated binary data.",
                    expected, actual
                )
            }
            SchemaError::UnexpectedEndOfData { context, position } => {
                write!(
                    f,
                    "Unexpected end of data at byte {}: {}.\n\
                     The encoded data appears to be truncated or corrupted.",
                    position, context
                )
            }
            SchemaError::InvalidVarint { context, position } => {
                write!(
                    f,
                    "Invalid varint encoding at byte {}: {}.\n\
                     Varint exceeded maximum 64-bit length or was malformed.",
                    position, context
                )
            }
            SchemaError::InvalidUtf8 { context, error } => {
                write!(
                    f,
                    "Invalid UTF-8 string while decoding {}: {}",
                    context, error
                )
            }
            SchemaError::TypeMismatch {
                field,
                expected,
                actual,
                row,
            } => {
                if let Some(row_num) = row {
                    write!(
                        f,
                        "Type mismatch in field '{}' at row {}: expected {}, but found {}.",
                        field, row_num, expected, actual
                    )
                } else {
                    write!(
                        f,
                        "Type mismatch in field '{}': expected {}, but found {}.",
                        field, expected, actual
                    )
                }
            }
            SchemaError::InvalidNullBitmap {
                expected_bytes,
                actual_bytes,
            } => {
                write!(
                    f,
                    "Invalid null bitmap size: expected {} bytes for null tracking, but found {}.\n\
                     This indicates corrupted header or bitmap data.",
                    expected_bytes, actual_bytes
                )
            }
            SchemaError::InvalidInput(msg) => write!(f, "{}", msg),
            SchemaError::InvalidFrame(msg) => write!(f, "{}", msg),
            SchemaError::InvalidCharacter(msg) => write!(f, "{}", msg),
            SchemaError::Compression(msg) => write!(f, "Compression error: {}", msg),
            SchemaError::Decompression(msg) => write!(f, "Decompression error: {}", msg),
            SchemaError::InvalidCompressionAlgorithm(algo) => write!(
                f,
                "Invalid compression algorithm byte: 0x{:02X}. Valid values are 0x00 (none), 0x01 (brotli), 0x02 (lz4), 0x03 (zstd).",
                algo
            ),
        }
    }
}

impl std::error::Error for SchemaError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_tags() {
        assert_eq!(FieldType::U64.type_tag(), 0);
        assert_eq!(FieldType::I64.type_tag(), 1);
        assert_eq!(FieldType::F64.type_tag(), 2);
        assert_eq!(FieldType::String.type_tag(), 3);
        assert_eq!(FieldType::Bool.type_tag(), 4);
        assert_eq!(FieldType::Null.type_tag(), 5);
        assert_eq!(FieldType::Array(Box::new(FieldType::U64)).type_tag(), 6);
        assert_eq!(FieldType::Any.type_tag(), 7);
    }

    #[test]
    fn test_field_type_from_tag() {
        assert_eq!(FieldType::from_type_tag(0, None).unwrap(), FieldType::U64);
        assert_eq!(FieldType::from_type_tag(1, None).unwrap(), FieldType::I64);
        assert_eq!(FieldType::from_type_tag(7, None).unwrap(), FieldType::Any);

        // Array requires element type
        assert!(FieldType::from_type_tag(6, None).is_err());
        assert_eq!(
            FieldType::from_type_tag(6, Some(Box::new(FieldType::U64))).unwrap(),
            FieldType::Array(Box::new(FieldType::U64))
        );

        // Invalid tag
        assert!(FieldType::from_type_tag(8, None).is_err());
    }

    #[test]
    fn test_schema_header_flags() {
        let mut header = SchemaHeader::new(10, vec![]);
        assert!(!header.has_flag(FLAG_TYPED_VALUES));

        header.set_flag(FLAG_TYPED_VALUES);
        assert!(header.has_flag(FLAG_TYPED_VALUES));
        assert!(!header.has_flag(FLAG_HAS_NULLS));

        header.set_flag(FLAG_HAS_NULLS);
        assert!(header.has_flag(FLAG_TYPED_VALUES));
        assert!(header.has_flag(FLAG_HAS_NULLS));
    }

    #[test]
    fn test_ir_creation() {
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

        let ir = IntermediateRepresentation::new(header, values).unwrap();
        assert_eq!(ir.header.row_count, 2);
        assert_eq!(ir.values.len(), 4);
    }

    #[test]
    fn test_ir_value_count_mismatch() {
        let fields = vec![
            FieldDef::new("id", FieldType::U64),
            FieldDef::new("name", FieldType::String),
        ];
        let header = SchemaHeader::new(2, fields);

        // Wrong number of values (should be 4, not 3)
        let values = vec![
            SchemaValue::U64(1),
            SchemaValue::String("Alice".to_string()),
            SchemaValue::U64(2),
        ];

        let result = IntermediateRepresentation::new(header, values);
        assert!(result.is_err());
        if let Err(SchemaError::ValueCountMismatch { expected, actual }) = result {
            assert_eq!(expected, 4);
            assert_eq!(actual, 3);
        } else {
            panic!("Expected ValueCountMismatch error");
        }
    }

    #[test]
    fn test_get_value() {
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

        let ir = IntermediateRepresentation::new(header, values).unwrap();

        assert_eq!(ir.get_value(0, 0), Some(&SchemaValue::U64(1)));
        assert_eq!(
            ir.get_value(0, 1),
            Some(&SchemaValue::String("Alice".to_string()))
        );
        assert_eq!(ir.get_value(1, 0), Some(&SchemaValue::U64(2)));
        assert_eq!(
            ir.get_value(1, 1),
            Some(&SchemaValue::String("Bob".to_string()))
        );
        assert_eq!(ir.get_value(2, 0), None); // Out of bounds
    }

    #[test]
    fn test_schema_value_type_matching() {
        assert!(SchemaValue::U64(42).matches_type(&FieldType::U64));
        assert!(!SchemaValue::U64(42).matches_type(&FieldType::I64));
        assert!(SchemaValue::U64(42).matches_type(&FieldType::Any));

        assert!(SchemaValue::String("test".to_string()).matches_type(&FieldType::String));
        assert!(SchemaValue::Bool(true).matches_type(&FieldType::Bool));
        assert!(SchemaValue::Null.matches_type(&FieldType::Null));
    }
}
