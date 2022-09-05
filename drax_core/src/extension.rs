#[cfg(feature = "footprints")]
use crate::transport::Footprint;
use crate::transport::{Error, Result, TransportProcessorContext};
use std::io::{Read, Write};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

macro_rules! read_var_number {
    ($var_type:ty, $bit_limit:literal, $($reader:tt)*) => {
        {
            let mut value = 0;
            let mut bit_offset = 0u32;

            loop {
                if bit_offset == $bit_limit {
                    return Error::cause("Var int too big, could not find end.");
                }

                let byte = {
                    $($reader)*
                };
                value |= <$var_type>::from(byte & 0b01111111)
                    .overflowing_shl(bit_offset)
                    .0;

                bit_offset += 7;
                if byte & 0b10000000 == 0 {
                    break;
                }
            }
            Ok(value)
        }
    };
}

macro_rules! declare_variable_number {
    ($var_type:ty, $unsigned_type:ty, $size_fn:ident, $read_async:ident, $read_sync:ident, $write_async:ident, $write_sync:ident, $bit_limit:literal, $and_check:literal) => {
        pub async fn $read_async<R: AsyncRead + Unpin>(
            _processor_context: &mut TransportProcessorContext,
            read: &mut R,
        ) -> Result<$var_type> {
            #[cfg(feature = "footprints")]
            _processor_context.mark(Footprint::note_type(stringify!($read_async)));
            read_var_number!($var_type, $bit_limit, read.read_u8().await?)
        }

        pub fn $read_sync<R: Read>(
            _processor_context: &mut TransportProcessorContext,
            read: &mut R,
        ) -> Result<$var_type> {
            #[cfg(feature = "footprints")]
            _processor_context.mark(Footprint::note_type(stringify!($read_sync)));
            read_var_number!($var_type, $bit_limit, {
                let mut byte = [0; 1];
                let read_amount = read.read(&mut byte)?;
                if read_amount == 0 {
                    return crate::transport::Error::cause("Invalid read, no byte ready.");
                }
                byte[0]
            })
        }

        pub fn $size_fn(
            var: $var_type,
            _processor_context: &mut TransportProcessorContext,
        ) -> Result<usize> {
            #[cfg(feature = "footprints")]
            _processor_context.mark(Footprint::note_type(stringify!($size_fn)));
            let mut temp: $unsigned_type = var as $unsigned_type;
            let mut size = 0;
            loop {
                if temp & $and_check == 0 {
                    return Ok(size + 1);
                }
                size += 1;
                temp = temp.overflowing_shr(7).0;
            }
        }

        pub async fn $write_async<W: AsyncWrite + Unpin>(
            var: $var_type,
            _processor_context: &mut TransportProcessorContext,
            writer: &mut W,
        ) -> Result<()> {
            #[cfg(feature = "footprints")]
            _processor_context.mark(Footprint::note_type(stringify!($write_async)));
            let mut temp: $unsigned_type = var as $unsigned_type;
            loop {
                if temp & $and_check == 0 {
                    writer.write_u8(temp as u8).await?;
                    return Ok(());
                }
                writer.write_u8((temp & 0x7F | 0x80) as u8).await?;
                temp = temp.overflowing_shr(7).0;
            }
        }

        pub fn $write_sync<W: Write>(
            var: $var_type,
            _processor_context: &mut TransportProcessorContext,
            writer: &mut W,
        ) -> Result<()> {
            #[cfg(feature = "footprints")]
            _processor_context.mark(Footprint::note_type(stringify!($write_sync)));
            let mut temp: $unsigned_type = var as $unsigned_type;
            loop {
                if temp & $and_check == 0 {
                    writer.write_all(&[temp as u8])?;
                    return Ok(());
                }
                writer.write_all(&[(temp & 0x7F | 0x80) as u8])?;
                temp = temp.overflowing_shr(7).0;
            }
        }
    };
}

declare_variable_number!(
    i32,
    u32,
    size_var_int,
    read_var_int,
    read_var_int_sync,
    write_var_int,
    write_var_int_sync,
    35,
    0xFFFFFF80
);

declare_variable_number!(
    i64,
    u64,
    size_var_long,
    read_var_long,
    read_var_long_sync,
    write_var_long,
    write_var_long_sync,
    70,
    0xFFFFFFFFFFFFFF80
);

pub fn write_string_checked<W: Write>(
    bytes: &[u8],
    context: &mut TransportProcessorContext,
    writer: &mut W,
) -> Result<()> {
    #[cfg(feature = "footprints")]
    context.mark(Footprint::note_type("write_string_checked"));

    write_var_int_sync(bytes.len() as i32, context, writer)?;
    writer.write_all(bytes)?;
    Ok(())
}

pub fn write_string<W: Write>(
    max_length: usize,
    string: &String,
    context: &mut TransportProcessorContext,
    writer: &mut W,
) -> Result<()> {
    #[cfg(feature = "footprints")]
    context.mark(Footprint::note_type("write_string"));
    let bytes = string.as_bytes();
    let length = bytes.len();
    if length > max_length * 3 {
        return Error::cause(format!(
            "Attempted to write string of length {} when max is {}.",
            length,
            max_length * 4
        ));
    }
    if length < 0 {
        return Error::cause(format!(
            "Cannot read a string of less than 0 length. Given {}.",
            length
        ));
    }
    write_string_checked(bytes, context, writer)
}

pub fn read_string_checked<R: Read>(
    length: usize,
    _context: &mut TransportProcessorContext,
    reader: &mut R,
) -> Result<String> {
    #[cfg(feature = "footprints")]
    _context.mark(Footprint::note_type("read_string_checked"));
    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes)?;
    let internal = String::from_utf8(bytes)?;
    Ok(internal)
}

pub fn read_string<R: Read>(
    max_length: usize,
    context: &mut TransportProcessorContext,
    reader: &mut R,
) -> Result<String> {
    #[cfg(feature = "footprints")]
    context.mark(Footprint::note_type("read_string"));
    let length = read_var_int_sync(context, reader)? as usize;
    if length > max_length * 3 {
        return Error::cause(format!(
            "Attempted to read string of length {} when max is {}.",
            length,
            max_length * 4
        ));
    }
    if length < 0 {
        return Error::cause(format!(
            "Cannot read a string of less than 0 length. Given {}.",
            length
        ));
    }
    read_string_checked(length, context, reader)
}

pub fn size_string(value: &String, context: &mut TransportProcessorContext) -> Result<usize> {
    #[cfg(feature = "footprints")]
    context.mark(Footprint::note_type("string size"));
    let string_len = value.len();
    Ok(size_var_int(string_len as i32, context)? + string_len)
}

pub fn write_json<T, W: Write>(
    max_length: usize,
    value: &T,
    context: &mut TransportProcessorContext,
    writer: &mut W,
) -> Result<()>
where
    T: serde::ser::Serialize,
{
    #[cfg(feature = "footprints")]
    context.mark(Footprint::note_type("write_json"));
    let value_to_string = serde_json::to_string(value)?;
    write_string(max_length, &value_to_string, context, writer)
}

pub fn size_json<T>(value: &T, context: &mut TransportProcessorContext) -> Result<usize>
where
    T: serde::ser::Serialize,
{
    #[cfg(feature = "footprints")]
    context.mark(Footprint::note_type("size_json"));
    let value_to_string = serde_json::to_string(value)?;
    size_string(&value_to_string, context)
}

pub fn read_json<T, R: Read>(
    max_length: usize,
    reader: &mut R,
    context: &mut TransportProcessorContext,
) -> Result<T>
where
    T: for<'de> serde::de::Deserialize<'de>,
{
    #[cfg(feature = "footprints")]
    context.mark(Footprint::note_type("read_json"));
    let json_string = read_string::<R>(max_length, context, reader)?;
    Ok(serde_json::from_slice(json_string.as_bytes())?)
}
