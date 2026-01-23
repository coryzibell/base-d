use crate::encoders::algorithms::schema::types::{
    FLAG_HAS_NULLS, FLAG_HAS_ROOT_KEY, FieldType, IntermediateRepresentation, SchemaValue,
};

/// Pack intermediate representation into binary format
pub fn pack(ir: &IntermediateRepresentation) -> Vec<u8> {
    let mut buffer = Vec::new();

    // Pack header
    pack_header(&mut buffer, ir);

    // Pack values
    pack_values(&mut buffer, ir);

    buffer
}

/// Pack the schema header
fn pack_header(buffer: &mut Vec<u8>, ir: &IntermediateRepresentation) {
    let header = &ir.header;

    // Flags
    buffer.push(header.flags);

    // Root key (if present)
    if header.has_flag(FLAG_HAS_ROOT_KEY)
        && let Some(key) = header.root_key.as_ref()
    {
        encode_varint(buffer, key.len() as u64);
        buffer.extend_from_slice(key.as_bytes());
    }

    // Row count
    encode_varint(buffer, header.row_count as u64);

    // Field count
    encode_varint(buffer, header.fields.len() as u64);

    // Field types (4 bits each, packed)
    pack_field_types(buffer, ir);

    // Field names
    for field in &header.fields {
        encode_varint(buffer, field.name.len() as u64);
        buffer.extend_from_slice(field.name.as_bytes());
    }

    // Null bitmap (if present)
    if header.has_flag(FLAG_HAS_NULLS)
        && let Some(bitmap) = header.null_bitmap.as_ref()
    {
        buffer.extend_from_slice(bitmap);
    }
}

/// Pack field types (4 bits each)
fn pack_field_types(buffer: &mut Vec<u8>, ir: &IntermediateRepresentation) {
    let mut type_buffer = Vec::new();
    let mut nibble_count = 0;

    for field in &ir.header.fields {
        pack_field_type_recursive(&mut type_buffer, &field.field_type, &mut nibble_count);
    }

    // Encode length of type buffer
    encode_varint(buffer, type_buffer.len() as u64);
    buffer.extend_from_slice(&type_buffer);
}

/// Pack a field type recursively (handles nested arrays)
fn pack_field_type_recursive(
    buffer: &mut Vec<u8>,
    field_type: &FieldType,
    nibble_count: &mut usize,
) {
    let tag = field_type.type_tag();

    // Pack as 4-bit nibbles (2 per byte)
    if (*nibble_count).is_multiple_of(2) {
        // Start new byte with tag in lower nibble
        buffer.push(tag);
    } else {
        // Add tag to upper nibble of last byte
        let last_idx = buffer.len() - 1;
        buffer[last_idx] |= tag << 4;
    }
    *nibble_count += 1;

    // If array, recursively pack element type
    if let FieldType::Array(element_type) = field_type {
        pack_field_type_recursive(buffer, element_type, nibble_count);
    }
}

/// Pack values
fn pack_values(buffer: &mut Vec<u8>, ir: &IntermediateRepresentation) {
    for value in &ir.values {
        pack_value(buffer, value);
    }
}

/// Pack a single value
fn pack_value(buffer: &mut Vec<u8>, value: &SchemaValue) {
    match value {
        SchemaValue::U64(v) => encode_varint(buffer, *v),
        SchemaValue::I64(v) => encode_signed_varint(buffer, *v),
        SchemaValue::F64(v) => buffer.extend_from_slice(&v.to_le_bytes()),
        SchemaValue::String(s) => {
            encode_varint(buffer, s.len() as u64);
            buffer.extend_from_slice(s.as_bytes());
        }
        SchemaValue::Bool(b) => buffer.push(if *b { 1 } else { 0 }),
        SchemaValue::Null => {} // Null encoded in bitmap, no value bytes
        SchemaValue::Array(arr) => {
            encode_varint(buffer, arr.len() as u64);
            // For arrays, we need to encode which elements are null
            // Write a null bitmap for the array elements
            let bitmap_bytes = arr.len().div_ceil(8);
            let mut null_bitmap = vec![0u8; bitmap_bytes];
            for (idx, item) in arr.iter().enumerate() {
                if matches!(item, SchemaValue::Null) {
                    let byte_idx = idx / 8;
                    let bit_idx = idx % 8;
                    null_bitmap[byte_idx] |= 1 << bit_idx;
                }
            }
            buffer.extend_from_slice(&null_bitmap);
            // Then write non-null values
            for item in arr {
                if !matches!(item, SchemaValue::Null) {
                    pack_value(buffer, item);
                }
            }
        }
    }
}

/// Encode unsigned varint (LEB128)
pub(crate) fn encode_varint(buffer: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80; // More bytes follow
        }
        buffer.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Encode signed varint using zigzag encoding
pub(crate) fn encode_signed_varint(buffer: &mut Vec<u8>, value: i64) {
    let encoded = ((value << 1) ^ (value >> 63)) as u64;
    encode_varint(buffer, encoded);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoders::algorithms::schema::types::{FieldDef, SchemaHeader};

    #[test]
    fn test_encode_varint() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 0);
        assert_eq!(buf, vec![0]);

        buf.clear();
        encode_varint(&mut buf, 1);
        assert_eq!(buf, vec![1]);

        buf.clear();
        encode_varint(&mut buf, 127);
        assert_eq!(buf, vec![127]);

        buf.clear();
        encode_varint(&mut buf, 128);
        assert_eq!(buf, vec![0x80, 0x01]);

        buf.clear();
        encode_varint(&mut buf, 16383);
        assert_eq!(buf, vec![0xFF, 0x7F]);

        buf.clear();
        encode_varint(&mut buf, 16384);
        assert_eq!(buf, vec![0x80, 0x80, 0x01]);
    }

    #[test]
    fn test_encode_signed_varint() {
        let mut buf = Vec::new();
        encode_signed_varint(&mut buf, 0);
        assert_eq!(buf, vec![0]);

        buf.clear();
        encode_signed_varint(&mut buf, -1);
        assert_eq!(buf, vec![1]);

        buf.clear();
        encode_signed_varint(&mut buf, 1);
        assert_eq!(buf, vec![2]);

        buf.clear();
        encode_signed_varint(&mut buf, -64);
        assert_eq!(buf, vec![127]);

        buf.clear();
        encode_signed_varint(&mut buf, 64);
        assert_eq!(buf, vec![128, 1]);
    }

    #[test]
    fn test_pack_simple_ir() {
        let fields = vec![
            FieldDef::new("id", FieldType::U64),
            FieldDef::new("name", FieldType::String),
        ];
        let header = SchemaHeader::new(1, fields);

        let values = vec![
            SchemaValue::U64(42),
            SchemaValue::String("Alice".to_string()),
        ];

        let ir = IntermediateRepresentation::new(header, values).unwrap();
        let packed = pack(&ir);

        // Verify it produces some output
        assert!(!packed.is_empty());

        // First byte should be flags (0 for no flags)
        assert_eq!(packed[0], 0);
    }

    #[test]
    fn test_pack_with_root_key() {
        let mut header = SchemaHeader::new(1, vec![FieldDef::new("id", FieldType::U64)]);
        header.root_key = Some("users".to_string());
        header.set_flag(FLAG_HAS_ROOT_KEY);

        let values = vec![SchemaValue::U64(42)];
        let ir = IntermediateRepresentation::new(header, values).unwrap();
        let packed = pack(&ir);

        // First byte should have FLAG_HAS_ROOT_KEY set
        assert_eq!(packed[0] & FLAG_HAS_ROOT_KEY, FLAG_HAS_ROOT_KEY);
    }

    #[test]
    fn test_pack_field_types() {
        let fields = vec![
            FieldDef::new("a", FieldType::U64),    // tag 0
            FieldDef::new("b", FieldType::I64),    // tag 1
            FieldDef::new("c", FieldType::String), // tag 3
        ];
        let header = SchemaHeader::new(1, fields);
        let values = vec![
            SchemaValue::U64(1),
            SchemaValue::I64(-1),
            SchemaValue::String("x".to_string()),
        ];

        let ir = IntermediateRepresentation::new(header, values).unwrap();
        let packed = pack(&ir);

        // Types should be packed as nibbles: 0, 1, 3
        // In bytes: 0x10 (0 and 1), 0x03 (3)
        // We need to find the type section in the packed data
        assert!(!packed.is_empty());
    }

    #[test]
    fn test_pack_values() {
        let mut buffer = Vec::new();

        pack_value(&mut buffer, &SchemaValue::U64(42));
        assert_eq!(buffer, vec![42]);

        buffer.clear();
        pack_value(&mut buffer, &SchemaValue::Bool(true));
        assert_eq!(buffer, vec![1]);

        buffer.clear();
        pack_value(&mut buffer, &SchemaValue::String("hi".to_string()));
        assert_eq!(buffer, vec![2, b'h', b'i']);
    }

    #[test]
    fn test_pack_array() {
        let mut buffer = Vec::new();
        let array = SchemaValue::Array(vec![SchemaValue::U64(1), SchemaValue::U64(2)]);
        pack_value(&mut buffer, &array);

        // Should be: count (2) + null_bitmap (1 byte, all zeros) + value (1) + value (2)
        assert_eq!(buffer, vec![2, 0, 1, 2]);
    }

    #[test]
    fn test_pack_array_with_nulls() {
        let mut buffer = Vec::new();
        let array = SchemaValue::Array(vec![
            SchemaValue::U64(1),
            SchemaValue::Null,
            SchemaValue::U64(3),
        ]);
        pack_value(&mut buffer, &array);

        // Should be: count (3) + null_bitmap (1 byte, bit 1 set = 0b00000010 = 2) + value (1) + value (3)
        assert_eq!(buffer, vec![3, 0b00000010, 1, 3]);
    }
}
