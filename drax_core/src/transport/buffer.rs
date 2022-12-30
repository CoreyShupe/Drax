pub mod var_num;

use crate::err_explain;
use crate::transport::buffer::var_num::{ReadVarInt, ReadVarLong, WriteVarInt, WriteVarLong};
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// A trait extension for `AsyncRead` which limits a stream.
pub trait Limiter {
    /// Limits the stream to a certain number of bytes. Error on overflow.
    fn hard_limit(&mut self, limit: usize) -> ReadLimiter<'_, Self>
    where
        Self: Sized;
}

impl<T> Limiter for T
where
    T: AsyncRead + Unpin,
{
    fn hard_limit(&mut self, limit: usize) -> ReadLimiter<'_, Self>
    where
        Self: Sized,
    {
        ReadLimiter::new(self, limit)
    }
}

/// A reader wrapper that limits the number of bytes that can be read from the underlying reader.
///
/// The `ReadLimiter` struct wraps an `AsyncRead` object and provides a method for limiting the number of bytes
/// that can be read from the underlying reader. When the limit is reached, any further read operations will return
/// an error with the message "Read limit exceeded". The `ReadLimiter` struct also provides a method for checking
/// that the entire specified number of bytes has been read from the reader.
pub struct ReadLimiter<'a, A> {
    reader: &'a mut A,
    limit: usize,
    current: usize,
}

impl<'a, A> ReadLimiter<'a, A> {
    /// Creates a new `ReadLimiter` that wraps the given reader and limits the number of bytes that can be read
    /// from the reader to the given number.
    ///
    /// # Parameters
    ///
    /// - `reader`: The reader to wrap.
    /// - `limit`: The maximum number of bytes that can be read from the reader.
    ///
    /// # Returns
    ///
    /// A new `ReadLimiter` that wraps the given reader and limits the number of bytes that can be read from the
    /// reader to the given number.
    ///
    /// # Examples
    ///
    /// A `ReadLimiter` will throw an error if a limit is reached, even prior to read:
    /// ```
    /// # use std::io::Cursor;
    /// # use tokio_test::{assert_err, assert_ok};
    /// # use drax::transport::buffer::ReadLimiter;
    /// # use tokio::io::AsyncReadExt;
    /// let mut cursor = Cursor::new(vec![1u8, 2, 3]);
    /// let mut limiter = ReadLimiter::new(&mut cursor, 2);
    /// let mut buf = [0; 3];
    /// assert_err!(tokio_test::block_on(async { limiter.read_exact(&mut buf).await }));
    /// ```
    ///
    /// If a read is exactly at the limit, no error will be thrown and it will pass through as expected.
    /// ```
    /// # use std::io::Cursor;
    /// # use tokio_test::{assert_err, assert_ok};
    /// # use drax::transport::buffer::ReadLimiter;
    /// # use tokio::io::AsyncReadExt;
    /// let mut cursor = Cursor::new(vec![1u8, 2, 3]);
    /// let mut limiter = ReadLimiter::new(&mut cursor, 2);
    /// let mut buf = [0; 2];
    /// assert_ok!(tokio_test::block_on(async { limiter.read_exact(&mut buf).await }));
    /// assert_eq!(buf, [1, 2]);
    /// ```
    pub fn new(reader: &'a mut A, limit: usize) -> Self {
        Self {
            reader,
            limit,
            current: 0,
        }
    }

    /// Checks that the entire specified number of bytes has been read from the reader.
    /// If the number of bytes read is less than the specified number, an error is returned.
    ///
    /// # Examples
    /// ```
    /// # use std::io::Cursor;
    /// # use tokio_test::{assert_err, assert_ok};
    /// # use drax::transport::buffer::ReadLimiter;
    /// # use tokio::io::AsyncReadExt;
    /// let mut cursor = Cursor::new(vec![1u8, 2, 3]);
    /// let mut limiter = ReadLimiter::new(&mut cursor, 2);
    /// let mut buf = [0; 1];
    /// assert_ok!(tokio_test::block_on(async { limiter.read_exact(&mut buf).await }));
    /// assert_err!(limiter.assert_length());
    /// ```
    pub fn assert_length(&self) -> crate::transport::Result<()> {
        if self.current == self.limit as usize {
            Ok(())
        } else {
            Err(err_explain!(
                "Buffer under-read, failed to read whole buffer"
            ))
        }
    }
}

impl<'a, A> AsyncRead for ReadLimiter<'a, A>
where
    A: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let filled_current = buf.filled().len();
        if self.current + buf.remaining() > self.limit as usize {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Read limit exceeded",
            )));
        }

        // if the remaining + the current is not greater than the limit then there's no way
        // we could possible read more bytes than the limit
        ready!(Pin::new(&mut *self.reader).poll_read(cx, buf))?;
        let filled = buf.filled().len() - filled_current;
        self.current += filled;

        Poll::Ready(Ok(()))
    }
}

/// Extension for reading common protocol types.
pub trait DraxReadExt {
    /// Reads a variable-length integer (VarInt) from the underlying reader.
    ///
    /// This function returns a future that reads a VarInt from the reader. A VarInt is an integer value that is
    /// encoded using a variable-length encoding that uses fewer bytes to represent smaller values and more bytes to
    /// represent larger values. This allows for more efficient storage and transmission of integers, especially when
    /// the majority of values are small.
    ///
    /// To read a VarInt, the future reads one byte at a time from the reader and combines them using bit shifts to
    /// form the final integer value. The process continues until a byte with the most significant bit (MSB) set to
    /// `0` is encountered, which indicates the end of the VarInt.
    ///
    /// If the end of the reader is reached before the VarInt is fully read, the future will return an error of
    /// type `ErrorType::EOF`. If the VarInt is too large to fit in the specified integer type, the future will
    /// return an error with the message "VarInt too large".
    ///
    /// # Parameters
    ///
    /// - `reader`: A mutable reference to the reader from which the VarInt will be read. The reader must implement
    ///   the `AsyncRead` trait.
    ///
    /// # Errors
    ///
    /// This function returns a future that may return an error when polled. The possible error values are:
    ///
    /// - `ErrorType::EOF`: If the end of the reader is reached before the VarInt is fully read.
    /// - `ErrorType::Generic`: If the VarInt is too large to fit in the specified integer type.
    ///
    /// # Returns
    ///
    /// A future that resolves to the VarInt value or an error if the VarInt could not be read. The future
    /// will return an error of type `ErrorType::EOF` if the end of the reader is reached before the VarInt
    /// is fully read. The future will return an error with the message "VarInt too large" if the VarInt
    /// is too large to fit in the specified integer type.
    fn read_var_int(&mut self) -> ReadVarInt<'_, Self>;

    /// Reads a variable-length long integer (VarLong) from the underlying reader.
    ///
    /// This function returns a future that reads a VarLong from the reader. A VarLong is a long value that is
    /// encoded using a variable-length encoding that uses fewer bytes to represent smaller values and more bytes to
    /// represent larger values. This allows for more efficient storage and transmission of integers, especially when
    /// the majority of values are small.
    ///
    /// To read a VarLong, the future reads one byte at a time from the reader and combines them using bit shifts to
    /// form the final long value. The process continues until a byte with the most significant bit (MSB) set to
    /// `0` is encountered, which indicates the end of the VarLong.
    ///
    /// If the end of the reader is reached before the VarLong is fully read, the future will return an error of
    /// type `ErrorType::EOF`. If the VarLong is too large to fit in the specified long type, the future will
    /// return an error with the message "VarLong too large".
    ///
    /// # Parameters
    ///
    /// - `reader`: A mutable reference to the reader from which the VarLong will be read. The reader must implement
    ///  the `AsyncRead` trait.
    ///
    /// # Errors
    ///
    /// This function returns a future that may return an error when polled. The possible error values are:
    ///
    /// - `ErrorType::EOF`: If the end of the reader is reached before the VarLong is fully read.
    /// - `ErrorType::Generic`: If the VarLong is too large to fit in the specified integer type.
    ///
    /// # Returns
    ///
    /// A future that resolves to the VarLong value or an error if the VarLong could not be read. The future
    /// will return an error of type `ErrorType::EOF` if the end of the reader is reached before the VarLong
    /// is fully read. The future will return an error with the message "VarLong too large" if the VarLong
    /// is too large to fit in the specified long type.
    fn read_var_long(&mut self) -> ReadVarLong<'_, Self>;
}

impl<T> DraxReadExt for T
where
    T: AsyncRead + Unpin + ?Sized,
{
    fn read_var_int(&mut self) -> ReadVarInt<'_, Self> {
        var_num::read_var_int(self)
    }

    fn read_var_long(&mut self) -> ReadVarLong<'_, Self> {
        var_num::read_var_long(self)
    }
}

/// Extension for writing common protocol types.
pub trait DraxWriteExt {
    /// Writes a variable-length integer (VarInt) to the underlying writer.
    ///
    /// This function returns a future that writes a VarInt to the writer. A VarInt is an integer value that is
    /// encoded using a variable-length encoding that uses fewer bytes to represent smaller values and more bytes to
    /// represent larger values. This allows for more efficient storage and transmission of integers, especially when
    /// the majority of values are small.
    ///
    /// To write a VarInt, the future writes one byte at a time to the writer. The integer value is split into 7-bit
    /// chunks and each chunk is written to the writer as a single byte. The most significant bit (MSB) of each byte
    /// is set to `1` except for the last byte, which has the MSB set to `0` to indicate the end of the VarInt.
    ///
    /// # Parameters
    ///
    /// - `writer`: A mutable reference to the writer to which the VarInt will be written. The writer must implement
    ///  the `AsyncWrite` trait.
    /// - `value`: The VarInt value to write.
    fn write_var_int(&mut self, value: i32) -> WriteVarInt<'_, Self>;

    /// Writes a variable-length long integer (VarLong) to the underlying writer.
    ///
    /// This function returns a future that writes a VarLong to the writer. A VarLong is a long value that is
    /// encoded using a variable-length encoding that uses fewer bytes to represent smaller values and more bytes to
    /// represent larger values. This allows for more efficient storage and transmission of integers, especially when
    /// the majority of values are small.
    ///
    /// To write a VarLong, the future writes one byte at a time to the writer. The long value is split into 7-bit
    /// chunks and each chunk is written to the writer as a single byte. The most significant bit (MSB) of each byte
    /// is set to `1` except for the last byte, which has the MSB set to `0` to indicate the end of the VarLong.
    ///
    /// # Parameters
    ///
    /// - `writer`: A mutable reference to the writer to which the VarLong will be written. The writer must implement
    /// the `AsyncWrite` trait.
    /// - `value`: The VarLong value to write.
    fn write_var_long(&mut self, value: i64) -> WriteVarLong<'_, Self>;
}

impl<T> DraxWriteExt for T
where
    T: AsyncWrite + Unpin + ?Sized,
{
    fn write_var_int(&mut self, value: i32) -> WriteVarInt<'_, Self> {
        var_num::write_var_int(self, value)
    }

    fn write_var_long(&mut self, value: i64) -> WriteVarLong<'_, Self> {
        var_num::write_var_long(self, value)
    }
}

#[cfg(test)]
mod tests {
    use super::{DraxReadExt, DraxWriteExt};
    use std::io::Cursor;

    // read ext

    macro_rules! var_int_tests {
        () => {
            vec![
                (25, vec![25]),
                (55324, vec![156, 176, 3]),
                (-8877777, vec![175, 146, 226, 251, 15]),
                (2147483647, vec![255, 255, 255, 255, 7]),
                (-2147483648, vec![128, 128, 128, 128, 8]),
            ]
        };
    }

    #[tokio::test]
    async fn test_read_var_int() -> crate::transport::Result<()> {
        for attempt in var_int_tests!() {
            let mut cursor = Cursor::new(attempt.1);
            let result = cursor.read_var_int().await?;
            assert_eq!(result, attempt.0);
        }
        Ok(())
    }

    // write ext

    #[tokio::test]
    async fn test_write_var_int() -> crate::transport::Result<()> {
        for attempt in var_int_tests!() {
            let mut cursor = Cursor::new(vec![]);
            cursor.write_var_int(attempt.0).await?;
            assert_eq!(cursor.into_inner(), attempt.1);
        }
        Ok(())
    }
}
