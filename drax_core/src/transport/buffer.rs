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
/// an error with the message "Read limit exceeded".
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

pub trait DraxReadExt {
    fn read_var_int(&mut self) -> ReadVarInt<'_, Self>;

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

pub trait DraxWriteExt {
    fn write_var_int(&mut self, value: i32) -> WriteVarInt<'_, Self>;

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
