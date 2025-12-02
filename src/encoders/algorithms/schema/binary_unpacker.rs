use crate::encoders::algorithms::schema::types::{
    FLAG_HAS_NULLS, FLAG_HAS_ROOT_KEY, FieldDef, FieldType, IntermediateRepresentation,
    SchemaError, SchemaHeader, SchemaValue,
};

/// Unpack binary data into intermediate representation
pub fn unpack(data: &[u8]) -> Result<IntermediateRepresentation, SchemaError> {
    let mut cursor = Cursor::new(data);

    // Unpack header
    let header = unpack_header(&mut cursor)?;

    // Unpack values
    let values = unpack_values(&mut cursor, &header)?;

    IntermediateRepresentation::new(header, values)
}

/// Simple cursor for tracking position in byte slice
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_byte(&mut self) -> Result<u8, SchemaError> {
        if self.pos >= self.data.len() {
            return Err(SchemaError::UnexpectedEndOfData {
                context: "reading single byte".to_string(),
                position: self.pos,
            });
        }
        let byte = self.data[self.pos];
        self.pos += 1;
        Ok(byte)
    }

    fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], SchemaError> {
        if self.remaining() < count {
            return Err(SchemaError::UnexpectedEndOfData {
                context: format!("reading {} bytes", count),
                position: self.pos,
            });
        }
        let bytes = &self.data[self.pos..self.pos + count];
        self.pos += count;
        Ok(bytes)
    }
}

/// Unpack the schema header
fn unpack_header(cursor: &mut Cursor) -> Result<SchemaHeader, SchemaError> {
    // Flags
    let flags = cursor.read_byte()?;

    // Root key (if present)
    let root_key = if flags & FLAG_HAS_ROOT_KEY != 0 {
        let len = decode_varint(cursor, "root key length")? as usize;
        let bytes = cursor.read_bytes(len)?;
        let key = String::from_utf8(bytes.to_vec()).map_err(|e| SchemaError::InvalidUtf8 {
            context: "root key".to_string(),
            error: e,
        })?;
        Some(key)
    } else {
        None
    };

    // Row count
    let row_count = decode_varint(cursor, "row count")? as usize;

    // Field count
    let field_count = decode_varint(cursor, "field count")? as usize;

    // Field types
    let fields = unpack_field_types(cursor, field_count)?;

    // Null bitmap (if present)
    let null_bitmap = if flags & FLAG_HAS_NULLS != 0 {
        let total_values = row_count * field_count;
        let bitmap_bytes = total_values.div_ceil(8);
        let bitmap = cursor.read_bytes(bitmap_bytes)?.to_vec();
        Some(bitmap)
    } else {
        None
    };

    Ok(SchemaHeader {
        flags,
        root_key,
        row_count,
        fields,
        null_bitmap,
        metadata: None, // Binary format doesn't preserve metadata (it's a fiche-only feature)
    })
}

/// Unpack field types
fn unpack_field_types(
    cursor: &mut Cursor,
    field_count: usize,
) -> Result<Vec<FieldDef>, SchemaError> {
    // Read type buffer length
    let type_buffer_len = decode_varint(cursor, "type buffer length")? as usize;
    let type_bytes = cursor.read_bytes(type_buffer_len)?;

    // Parse field types from nibbles
    let mut types = Vec::new();
    let mut nibble_cursor = NibbleCursor::new(type_bytes);

    for i in 0..field_count {
        let field_type = unpack_field_type_recursive(&mut nibble_cursor, i)?;
        types.push(field_type);
    }

    // Read field names
    let mut fields = Vec::new();
    for (idx, field_type) in types.into_iter().enumerate() {
        let name_len = decode_varint(cursor, &format!("field {} name length", idx))? as usize;
        let name_bytes = cursor.read_bytes(name_len)?;
        let name =
            String::from_utf8(name_bytes.to_vec()).map_err(|e| SchemaError::InvalidUtf8 {
                context: format!("field {} name", idx),
                error: e,
            })?;
        fields.push(FieldDef::new(name, field_type));
    }

    Ok(fields)
}

/// Cursor for reading 4-bit nibbles from bytes
struct NibbleCursor<'a> {
    bytes: &'a [u8],
    pos: usize, // Position in bytes
    high: bool, // true = read high nibble next, false = read low nibble next
}

impl<'a> NibbleCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            high: false, // Start with low nibble
        }
    }

    fn read_nibble(&mut self) -> Result<u8, SchemaError> {
        if self.pos >= self.bytes.len() {
            return Err(SchemaError::UnexpectedEndOfData {
                context: "reading type tag nibble".to_string(),
                position: self.pos,
            });
        }

        let byte = self.bytes[self.pos];
        let nibble = if self.high { byte >> 4 } else { byte & 0x0F };

        if self.high {
            self.pos += 1;
            self.high = false;
        } else {
            self.high = true;
        }

        Ok(nibble)
    }
}

/// Unpack a field type recursively
fn unpack_field_type_recursive(
    cursor: &mut NibbleCursor,
    field_index: usize,
) -> Result<FieldType, SchemaError> {
    let tag = cursor.read_nibble()?;

    if tag == 6 {
        // Array type - recursively read element type
        let element_type = Box::new(unpack_field_type_recursive(cursor, field_index)?);
        FieldType::from_type_tag(tag, Some(element_type)).map_err(|e| match e {
            SchemaError::InvalidTypeTag { tag, .. } => SchemaError::InvalidTypeTag {
                tag,
                context: Some(format!("field {} type definition", field_index)),
            },
            other => other,
        })
    } else {
        FieldType::from_type_tag(tag, None).map_err(|e| match e {
            SchemaError::InvalidTypeTag { tag, .. } => SchemaError::InvalidTypeTag {
                tag,
                context: Some(format!("field {} type definition", field_index)),
            },
            other => other,
        })
    }
}

/// Unpack values
fn unpack_values(
    cursor: &mut Cursor,
    header: &SchemaHeader,
) -> Result<Vec<SchemaValue>, SchemaError> {
    let mut values = Vec::new();
    let total_values = header.row_count * header.fields.len();

    for i in 0..total_values {
        let field_idx = i % header.fields.len();
        let field_type = &header.fields[field_idx].field_type;

        // Check if value is null
        if let Some(ref bitmap) = header.null_bitmap {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            if byte_idx < bitmap.len() && (bitmap[byte_idx] >> bit_idx) & 1 == 1 {
                values.push(SchemaValue::Null);
                continue;
            }
        }

        let value = unpack_value(cursor, field_type)?;
        values.push(value);
    }

    Ok(values)
}

/// Unpack a single value
fn unpack_value(cursor: &mut Cursor, field_type: &FieldType) -> Result<SchemaValue, SchemaError> {
    match field_type {
        FieldType::U64 => {
            let v = decode_varint(cursor, "u64 value")?;
            Ok(SchemaValue::U64(v))
        }
        FieldType::I64 => {
            let v = decode_signed_varint(cursor, "i64 value")?;
            Ok(SchemaValue::I64(v))
        }
        FieldType::F64 => {
            let bytes = cursor.read_bytes(8)?;
            let v = f64::from_le_bytes(bytes.try_into().unwrap());
            Ok(SchemaValue::F64(v))
        }
        FieldType::String => {
            let len = decode_varint(cursor, "string length")? as usize;
            let bytes = cursor.read_bytes(len)?;
            let s = String::from_utf8(bytes.to_vec()).map_err(|e| SchemaError::InvalidUtf8 {
                context: "string value".to_string(),
                error: e,
            })?;
            Ok(SchemaValue::String(s))
        }
        FieldType::Bool => {
            let byte = cursor.read_byte()?;
            Ok(SchemaValue::Bool(byte != 0))
        }
        FieldType::Null => Ok(SchemaValue::Null),
        FieldType::Array(element_type) => {
            let count = decode_varint(cursor, "array element count")? as usize;
            let mut arr = Vec::new();
            for _ in 0..count {
                let item = unpack_value(cursor, element_type)?;
                arr.push(item);
            }
            Ok(SchemaValue::Array(arr))
        }
        FieldType::Any => {
            // Read type tag byte
            let tag = cursor.read_byte()?;
            let temp_type = FieldType::from_type_tag(tag & 0x0F, None)?;
            unpack_value(cursor, &temp_type)
        }
    }
}

/// Decode unsigned varint (LEB128)
fn decode_varint(cursor: &mut Cursor, context: &str) -> Result<u64, SchemaError> {
    let start_pos = cursor.pos;
    let mut result = 0u64;
    let mut shift = 0;

    loop {
        if shift >= 64 {
            return Err(SchemaError::InvalidVarint {
                context: context.to_string(),
                position: start_pos,
            });
        }

        let byte = cursor.read_byte()?;
        result |= ((byte & 0x7F) as u64) << shift;
        shift += 7;

        if byte & 0x80 == 0 {
            break;
        }
    }

    Ok(result)
}

/// Decode signed varint using zigzag decoding
fn decode_signed_varint(cursor: &mut Cursor, context: &str) -> Result<i64, SchemaError> {
    let encoded = decode_varint(cursor, context)?;
    let decoded = ((encoded >> 1) as i64) ^ (-((encoded & 1) as i64));
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_varint() {
        let data = vec![0];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_varint(&mut cursor, "test").unwrap(), 0);

        let data = vec![1];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_varint(&mut cursor, "test").unwrap(), 1);

        let data = vec![127];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_varint(&mut cursor, "test").unwrap(), 127);

        let data = vec![0x80, 0x01];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_varint(&mut cursor, "test").unwrap(), 128);

        let data = vec![0xFF, 0x7F];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_varint(&mut cursor, "test").unwrap(), 16383);

        let data = vec![0x80, 0x80, 0x01];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_varint(&mut cursor, "test").unwrap(), 16384);
    }

    #[test]
    fn test_decode_signed_varint() {
        let data = vec![0];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_signed_varint(&mut cursor, "test").unwrap(), 0);

        let data = vec![1];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_signed_varint(&mut cursor, "test").unwrap(), -1);

        let data = vec![2];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_signed_varint(&mut cursor, "test").unwrap(), 1);

        let data = vec![127];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_signed_varint(&mut cursor, "test").unwrap(), -64);

        let data = vec![128, 1];
        let mut cursor = Cursor::new(&data);
        assert_eq!(decode_signed_varint(&mut cursor, "test").unwrap(), 64);
    }

    #[test]
    fn test_round_trip_varint() {
        use crate::encoders::algorithms::schema::binary_packer;

        for value in [0, 1, 127, 128, 16383, 16384, 1000000] {
            let mut buf = Vec::new();
            binary_packer::encode_varint(&mut buf, value);

            let mut cursor = Cursor::new(&buf);
            let decoded = decode_varint(&mut cursor, "test").unwrap();
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn test_round_trip_signed_varint() {
        use crate::encoders::algorithms::schema::binary_packer;

        for value in [-1000, -64, -1, 0, 1, 64, 1000] {
            let mut buf = Vec::new();
            binary_packer::encode_signed_varint(&mut buf, value);

            let mut cursor = Cursor::new(&buf);
            let decoded = decode_signed_varint(&mut cursor, "test").unwrap();
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn test_nibble_cursor() {
        let data = vec![0x10, 0x32]; // nibbles: 0, 1, 2, 3
        let mut cursor = NibbleCursor::new(&data);

        assert_eq!(cursor.read_nibble().unwrap(), 0);
        assert_eq!(cursor.read_nibble().unwrap(), 1);
        assert_eq!(cursor.read_nibble().unwrap(), 2);
        assert_eq!(cursor.read_nibble().unwrap(), 3);
    }
}
