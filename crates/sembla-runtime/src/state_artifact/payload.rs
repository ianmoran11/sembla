use super::*;

pub(super) fn write_column_bytes(
    output: &mut impl std::io::Write,
    data: &ColumnData,
) -> std::io::Result<()> {
    match data {
        ColumnData::Real(values) => {
            write_values(output, values, |value| value.to_bits().to_le_bytes())
        }
        ColumnData::Int(values) => write_values(output, values, |value| value.to_le_bytes()),
        ColumnData::Enum(values) => write_values(output, values, |value| value.to_le_bytes()),
        ColumnData::Ref(values) => write_values(output, values, |value| value.to_le_bytes()),
    }
}

fn write_values<T, const WIDTH: usize>(
    output: &mut impl std::io::Write,
    values: &[T],
    encode: impl Fn(&T) -> [u8; WIDTH],
) -> std::io::Result<()> {
    const CHUNK_VALUES: usize = 8_192;
    for chunk in values.chunks(CHUNK_VALUES) {
        let mut bytes = Vec::with_capacity(chunk.len() * WIDTH);
        for value in chunk {
            bytes.extend_from_slice(&encode(value));
        }
        output.write_all(&bytes)?;
    }
    Ok(())
}

pub(super) fn decode_column(column_type: ColumnType, bytes: &[u8]) -> ColumnData {
    match column_type {
        ColumnType::Real => ColumnData::Real(decode_values(bytes, |value| {
            f64::from_bits(u64::from_le_bytes(value))
        })),
        ColumnType::Int => ColumnData::Int(decode_values(bytes, i64::from_le_bytes)),
        ColumnType::Enum => ColumnData::Enum(decode_values(bytes, u16::from_le_bytes)),
        ColumnType::Ref => ColumnData::Ref(decode_values(bytes, u32::from_le_bytes)),
    }
}

fn decode_values<T, const WIDTH: usize>(bytes: &[u8], decode: impl Fn([u8; WIDTH]) -> T) -> Vec<T> {
    bytes
        .chunks_exact(WIDTH)
        .map(|chunk| decode(chunk.try_into().expect("fixed-width column value")))
        .collect()
}
