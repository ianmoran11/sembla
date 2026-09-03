use super::*;

// The header has a closed schema, so its lexically sorted key order is fixed.
// Every variable JSON string still goes through the IR's canonical serializer;
// the only numbers are unsigned decimal counts. This keeps the runtime's
// dependency surface narrow while sharing the canonical string/escape rules.
pub(super) fn canonical_header(header: &Header) -> Result<String, StateArtifactError> {
    use fmt::Write as _;

    let mut output = String::new();
    output.push_str("{\"schema_version\":");
    output.push_str(&canonical_json_string(&header.schema_version)?);
    output.push_str(",\"tables\":[");
    for (table_index, table) in header.tables.iter().enumerate() {
        if table_index != 0 {
            output.push(',');
        }
        output.push_str("{\"box\":");
        output.push_str(&canonical_json_string(&table.box_name)?);
        output.push_str(",\"columns\":[");
        for (column_index, column) in table.columns.iter().enumerate() {
            if column_index != 0 {
                output.push(',');
            }
            output.push_str("{\"name\":");
            output.push_str(&canonical_json_string(&column.name)?);
            if let Some(target) = &column.ref_target {
                output.push_str(",\"ref_target\":{\"box\":");
                output.push_str(&canonical_json_string(&target.box_name)?);
                output.push_str(",\"table\":");
                output.push_str(&canonical_json_string(&target.table)?);
                output.push('}');
            }
            output.push_str(",\"type\":");
            output.push_str(&canonical_json_string(column.column_type.name())?);
            if let Some(variant_count) = column.variant_count {
                write!(&mut output, ",\"variant_count\":{variant_count}")
                    .expect("writing to String cannot fail");
            }
            output.push('}');
        }
        write!(
            &mut output,
            "],\"row_count\":{},\"table\":",
            table.row_count
        )
        .expect("writing to String cannot fail");
        output.push_str(&canonical_json_string(&table.table)?);
        output.push('}');
    }
    output.push_str("]}");
    Ok(output)
}

fn canonical_json_string(value: &str) -> Result<String, StateArtifactError> {
    to_canonical_string(&value).map_err(StateArtifactError::InvalidHeaderJson)
}

#[derive(Clone, Debug)]
enum JsonValue {
    Object(Vec<(String, JsonValue)>),
    Array(Vec<JsonValue>),
    String(String),
    Number(u64),
    Bool,
    Null,
}

// Strict parser for the closed header schema. It accepts semantically valid
// non-canonical JSON so the caller can distinguish it from malformed JSON by
// re-emitting and comparing the exact canonical header bytes.
struct JsonParser<'a> {
    source: &'a str,
    offset: usize,
}

impl<'a> JsonParser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, offset: 0 }
    }

    fn parse(mut self) -> Result<JsonValue, StateArtifactError> {
        let value = self.parse_value()?;
        self.skip_whitespace();
        if self.offset != self.source.len() {
            return self.error("trailing characters after header value");
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JsonValue, StateArtifactError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'\"') => self.parse_string().map(JsonValue::String),
            Some(b'0'..=b'9') => self.parse_number().map(JsonValue::Number),
            Some(b't') => {
                self.literal("true")?;
                Ok(JsonValue::Bool)
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(JsonValue::Bool)
            }
            Some(b'n') => {
                self.literal("null")?;
                Ok(JsonValue::Null)
            }
            Some(other) => self.error(&format!("unexpected byte 0x{other:02x}")),
            None => self.error("unexpected end of header"),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, StateArtifactError> {
        self.offset += 1;
        self.skip_whitespace();
        let mut fields = Vec::new();
        if self.consume(b'}') {
            return Ok(JsonValue::Object(fields));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'\"') {
                return self.error("object key must be a string");
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            if !self.consume(b':') {
                return self.error("object key must be followed by ':'");
            }
            fields.push((key, self.parse_value()?));
            self.skip_whitespace();
            if self.consume(b'}') {
                break;
            }
            if !self.consume(b',') {
                return self.error("object entries must be separated by ','");
            }
        }
        Ok(JsonValue::Object(fields))
    }

    fn parse_array(&mut self) -> Result<JsonValue, StateArtifactError> {
        self.offset += 1;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume(b']') {
            return Ok(JsonValue::Array(values));
        }
        loop {
            values.push(self.parse_value()?);
            self.skip_whitespace();
            if self.consume(b']') {
                break;
            }
            if !self.consume(b',') {
                return self.error("array entries must be separated by ','");
            }
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_string(&mut self) -> Result<String, StateArtifactError> {
        debug_assert_eq!(self.peek(), Some(b'\"'));
        self.offset += 1;
        let mut output = String::new();
        loop {
            let byte = self
                .peek()
                .ok_or_else(|| header_json_error(self.offset, "unterminated string"))?;
            match byte {
                b'\"' => {
                    self.offset += 1;
                    return Ok(output);
                }
                b'\\' => {
                    self.offset += 1;
                    let escape = self
                        .peek()
                        .ok_or_else(|| header_json_error(self.offset, "unterminated escape"))?;
                    self.offset += 1;
                    match escape {
                        b'\"' => output.push('\"'),
                        b'\\' => output.push('\\'),
                        b'/' => output.push('/'),
                        b'b' => output.push('\u{0008}'),
                        b'f' => output.push('\u{000c}'),
                        b'n' => output.push('\n'),
                        b'r' => output.push('\r'),
                        b't' => output.push('\t'),
                        b'u' => output.push(self.parse_unicode_escape()?),
                        _ => return self.error("invalid string escape"),
                    }
                }
                0x00..=0x1f => return self.error("unescaped control character in string"),
                0x20..=0x7f => {
                    output.push(char::from(byte));
                    self.offset += 1;
                }
                _ => {
                    let value = self.source[self.offset..]
                        .chars()
                        .next()
                        .ok_or_else(|| header_json_error(self.offset, "invalid UTF-8 string"))?;
                    output.push(value);
                    self.offset += value.len_utf8();
                }
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, StateArtifactError> {
        let first = self.parse_hex_quad()?;
        if (0xd800..=0xdbff).contains(&first) {
            if !self.consume(b'\\') || !self.consume(b'u') {
                return self.error("high surrogate must be followed by a low surrogate");
            }
            let second = self.parse_hex_quad()?;
            if !(0xdc00..=0xdfff).contains(&second) {
                return self.error("invalid low surrogate");
            }
            let scalar =
                0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00);
            char::from_u32(scalar)
                .ok_or_else(|| header_json_error(self.offset, "invalid Unicode scalar"))
        } else if (0xdc00..=0xdfff).contains(&first) {
            self.error("unpaired low surrogate")
        } else {
            char::from_u32(u32::from(first))
                .ok_or_else(|| header_json_error(self.offset, "invalid Unicode scalar"))
        }
    }

    fn parse_hex_quad(&mut self) -> Result<u16, StateArtifactError> {
        let mut value = 0u16;
        for _ in 0..4 {
            let byte = self
                .peek()
                .ok_or_else(|| header_json_error(self.offset, "truncated Unicode escape"))?;
            self.offset += 1;
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte - b'0'),
                b'a'..=b'f' => u16::from(byte - b'a' + 10),
                b'A'..=b'F' => u16::from(byte - b'A' + 10),
                _ => return self.error("invalid hexadecimal Unicode escape"),
            };
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<u64, StateArtifactError> {
        let start = self.offset;
        if self.consume(b'0') {
            if matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.error("number has a leading zero");
            }
        } else {
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.offset += 1;
            }
        }
        self.source[start..self.offset]
            .parse::<u64>()
            .map_err(|_| header_json_error(start, "number is outside u64 range"))
    }

    fn literal(&mut self, literal: &str) -> Result<(), StateArtifactError> {
        if self.source[self.offset..].starts_with(literal) {
            self.offset += literal.len();
            Ok(())
        } else {
            self.error("invalid JSON literal")
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.offset += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.offset).copied()
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn error<T>(&self, message: &str) -> Result<T, StateArtifactError> {
        Err(header_json_error(self.offset, message))
    }
}

fn header_json_error(offset: usize, message: &str) -> StateArtifactError {
    StateArtifactError::InvalidHeaderJson(format!("byte {offset}: {message}"))
}

pub(super) fn parse_header(source: &str) -> Result<Header, StateArtifactError> {
    let mut fields = object_fields(JsonParser::new(source).parse()?, "header")?;
    let schema_version = take_string(
        required(&mut fields, "schema_version", "header")?,
        "header.schema_version",
    )?;
    let tables = take_array(required(&mut fields, "tables", "header")?, "header.tables")?
        .into_iter()
        .enumerate()
        .map(|(index, table)| parse_table(table, index))
        .collect::<Result<Vec<_>, _>>()?;
    reject_extra_fields(&fields, "header")?;
    Ok(Header {
        schema_version,
        tables,
    })
}

fn parse_table(value: JsonValue, index: usize) -> Result<TableHeader, StateArtifactError> {
    let context = format!("header.tables[{index}]");
    let mut fields = object_fields(value, &context)?;
    let box_name = take_string(
        required(&mut fields, "box", &context)?,
        &format!("{context}.box"),
    )?;
    let table = take_string(
        required(&mut fields, "table", &context)?,
        &format!("{context}.table"),
    )?;
    let row_count = take_number(
        required(&mut fields, "row_count", &context)?,
        &format!("{context}.row_count"),
    )?;
    let columns = take_array(
        required(&mut fields, "columns", &context)?,
        &format!("{context}.columns"),
    )?
    .into_iter()
    .enumerate()
    .map(|(column_index, column)| parse_column(column, index, column_index))
    .collect::<Result<Vec<_>, _>>()?;
    reject_extra_fields(&fields, &context)?;
    Ok(TableHeader {
        box_name,
        table,
        row_count,
        columns,
    })
}

fn parse_column(
    value: JsonValue,
    table_index: usize,
    column_index: usize,
) -> Result<ColumnHeader, StateArtifactError> {
    let context = format!("header.tables[{table_index}].columns[{column_index}]");
    let mut fields = object_fields(value, &context)?;
    let name = take_string(
        required(&mut fields, "name", &context)?,
        &format!("{context}.name"),
    )?;
    let type_name = take_string(
        required(&mut fields, "type", &context)?,
        &format!("{context}.type"),
    )?;
    let column_type = match type_name.as_str() {
        "real" => ColumnType::Real,
        "int" => ColumnType::Int,
        "enum" => ColumnType::Enum,
        "ref" => ColumnType::Ref,
        _ => {
            return Err(header_json_error(
                0,
                &format!("{context}.type has unsupported value '{type_name}'"),
            ))
        }
    };
    let variant_count = fields
        .remove("variant_count")
        .map(|value| take_number(value, &format!("{context}.variant_count")))
        .transpose()?;
    let ref_target = fields
        .remove("ref_target")
        .map(|value| parse_ref_target(value, &format!("{context}.ref_target")))
        .transpose()?;
    reject_extra_fields(&fields, &context)?;
    Ok(ColumnHeader {
        name,
        column_type,
        variant_count,
        ref_target,
    })
}

fn parse_ref_target(value: JsonValue, context: &str) -> Result<RefTarget, StateArtifactError> {
    let mut fields = object_fields(value, context)?;
    let box_name = take_string(
        required(&mut fields, "box", context)?,
        &format!("{context}.box"),
    )?;
    let table = take_string(
        required(&mut fields, "table", context)?,
        &format!("{context}.table"),
    )?;
    reject_extra_fields(&fields, context)?;
    Ok(RefTarget { box_name, table })
}

fn object_fields(
    value: JsonValue,
    context: &str,
) -> Result<BTreeMap<String, JsonValue>, StateArtifactError> {
    let JsonValue::Object(entries) = value else {
        return Err(header_json_error(
            0,
            &format!("{context} must be an object"),
        ));
    };
    let mut fields = BTreeMap::new();
    for (name, value) in entries {
        if fields.insert(name.clone(), value).is_some() {
            return Err(header_json_error(
                0,
                &format!("{context} has duplicate field '{name}'"),
            ));
        }
    }
    Ok(fields)
}

fn required(
    fields: &mut BTreeMap<String, JsonValue>,
    name: &str,
    context: &str,
) -> Result<JsonValue, StateArtifactError> {
    fields
        .remove(name)
        .ok_or_else(|| header_json_error(0, &format!("{context} is missing field '{name}'")))
}

fn reject_extra_fields(
    fields: &BTreeMap<String, JsonValue>,
    context: &str,
) -> Result<(), StateArtifactError> {
    if let Some(name) = fields.keys().next() {
        Err(header_json_error(
            0,
            &format!("{context} has unknown field '{name}'"),
        ))
    } else {
        Ok(())
    }
}

fn take_string(value: JsonValue, context: &str) -> Result<String, StateArtifactError> {
    if let JsonValue::String(value) = value {
        Ok(value)
    } else {
        Err(header_json_error(0, &format!("{context} must be a string")))
    }
}

fn take_number(value: JsonValue, context: &str) -> Result<u64, StateArtifactError> {
    if let JsonValue::Number(value) = value {
        Ok(value)
    } else {
        Err(header_json_error(
            0,
            &format!("{context} must be an unsigned integer"),
        ))
    }
}

fn take_array(value: JsonValue, context: &str) -> Result<Vec<JsonValue>, StateArtifactError> {
    if let JsonValue::Array(value) = value {
        Ok(value)
    } else {
        Err(header_json_error(0, &format!("{context} must be an array")))
    }
}
