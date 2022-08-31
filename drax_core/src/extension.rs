#[cfg(feature = "footprints")]
use crate::transport::Footprint;
use crate::transport::{Error, TransportProcessorContext};
use tokio::io::{AsyncRead, AsyncReadExt};

/// Reads in a var int from a given [tokio::io::AsyncRead].
///
/// Var ints are used to reduce the size of packets by not sending known-empty bytes of integers.
/// Using `0b10000000` as a flag we can encode a "continue" into bytes by reading right to left.
/// Var ints are at most 5 bytes and at least 1 byte.
///
/// Each var int is read from the right most byte to the left most byte so that:
/// ```rust
/// # use std::io::Cursor;
/// # use drax::extension::read_var_int;
/// # use drax::transport::TransportProcessorContext;
/// # let mut process_context = TransportProcessorContext::new();
/// # let proc = async move {
///     let mut input = Cursor::new(vec![128, 1]);
///     let output = read_var_int(&mut process_context, &mut input).await.expect("VarInt");
///     assert_eq!(output, 128);
/// # };
/// # tokio_test::block_on(proc);
/// ```
///
/// This format supports the entire range of integers, from -2147483648 to 2147483647.
pub async fn read_var_int<R: AsyncRead + Unpin>(
    processor_context: &mut TransportProcessorContext,
    read: &mut R,
) -> crate::transport::Result<i32> {
    #[cfg(feature = "footprints")]
    processor_context.mark(Footprint::note_type("read VarInt"));

    let mut value = 0;
    let mut bit_offset = 0u32;

    loop {
        if bit_offset == 0xFFFFFF80 {
            return Error::cause("Var int too big, could not find end.");
        }

        let byte = read.read_u8().await?;
        value |= i32::from(byte & 0b01111111).overflowing_shl(bit_offset).0;

        bit_offset += 7;
        if byte & 0b10000000 == 0 {
            break;
        }
    }
    Ok(value)
}
