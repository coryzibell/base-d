use std::fmt;

/// Field types supported in schema encoding
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
                    Err(SchemaError::InvalidTypeTag(tag))
                }
            }
            7 => Ok(FieldType::Any),
            _ => Err(SchemaError::InvalidTypeTag(tag)),
        }
    }
}

/// Header flags (bit positions)
#[allow(dead_code)]
pub const FLAG_TYPED_VALUES: u8 = 0b0000_0001; // Per-value type tags
pub const FLAG_HAS_NULLS: u8 = 0b0000_0010; // Null bitmap present
pub const FLAG_HAS_ROOT_KEY: u8 = 0b0000_0100; // Root key in header

/// Schema header
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaHeader {
    pub flags: u8,
    pub root_key: Option<String>,
    pub row_count: usize,
    pub fields: Vec<FieldDef>,
    pub null_bitmap: Option<Vec<u8>>,
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

/// Field definition
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

/// Schema value
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
#[derive(Debug, PartialEq)]
pub enum SchemaError {
    /// Invalid type tag encountered
    InvalidTypeTag(u8),
    /// Value count mismatch
    ValueCountMismatch { expected: usize, actual: usize },
    /// Unexpected end of data
    UnexpectedEndOfData,
    /// Invalid varint encoding
    InvalidVarint,
    /// Invalid UTF-8 string
    InvalidUtf8(std::string::FromUtf8Error),
    /// Type mismatch between value and field type
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
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
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaError::InvalidTypeTag(tag) => write!(f, "invalid type tag: {}", tag),
            SchemaError::ValueCountMismatch { expected, actual } => {
                write!(
                    f,
                    "value count mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            SchemaError::UnexpectedEndOfData => write!(f, "unexpected end of data"),
            SchemaError::InvalidVarint => write!(f, "invalid varint encoding"),
            SchemaError::InvalidUtf8(e) => write!(f, "invalid UTF-8 string: {}", e),
            SchemaError::TypeMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "type mismatch for field '{}': expected {}, got {}",
                field, expected, actual
            ),
            SchemaError::InvalidNullBitmap {
                expected_bytes,
                actual_bytes,
            } => write!(
                f,
                "invalid null bitmap: expected {} bytes, got {}",
                expected_bytes, actual_bytes
            ),
            SchemaError::InvalidInput(msg) => write!(f, "invalid input: {}", msg),
            SchemaError::InvalidFrame(msg) => write!(f, "invalid frame: {}", msg),
            SchemaError::InvalidCharacter(msg) => write!(f, "invalid character: {}", msg),
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
        assert!(matches!(
            result,
            Err(SchemaError::ValueCountMismatch {
                expected: 4,
                actual: 3
            })
        ));
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
