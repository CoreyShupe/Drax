use crate::err_explain;
use crate::prelude::OwnedPacketComponent;
use crate::transport::buffer::var_num::{ReadVarInt, ReadVarLong, WriteVarInt, WriteVarLong};
#[cfg(feature = "encryption")]
use crate::transport::encryption::{DecryptRead, Decryption, EncryptedWriter, Encryption};
use std::future::Future;
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

    #[cfg(feature = "encryption")]
    fn no_crypt_wrap(self) -> DecryptRead<Self>
    where
        Self: Sized;

    #[cfg(feature = "encryption")]
    fn decrypt_stream(self, decryption: Decryption) -> DecryptRead<Self>
    where
        Self: Sized;

    fn decode_packet<'a, P: OwnedPacketComponent>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<P>> + 'a>>
    where
        P: Sized;
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

    #[cfg(feature = "encryption")]
    fn no_crypt_wrap(self) -> DecryptRead<Self>
    where
        Self: Sized,
    {
        DecryptRead::noop(self)
    }

    #[cfg(feature = "encryption")]
    fn decrypt_stream(self, decryption: Decryption) -> DecryptRead<Self>
    where
        Self: Sized,
    {
        DecryptRead::new(self, decryption)
    }

    fn decode_packet<'a, P: OwnedPacketComponent>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<P>> + 'a>>
    where
        P: Sized,
    {
        P::decode_owned(self)
    }
}

pub trait DraxWriteExt {
    fn write_var_int(&mut self, value: i32) -> WriteVarInt<'_, Self>;

    fn write_var_long(&mut self, value: i64) -> WriteVarLong<'_, Self>;

    #[cfg(feature = "encryption")]
    fn no_crypt_wrap(self) -> EncryptedWriter<Self>
    where
        Self: Sized;

    #[cfg(feature = "encryption")]
    fn encrypt_stream(self, encryption: Encryption) -> EncryptedWriter<Self>
    where
        Self: Sized;

    fn encode_packet<'a, P: OwnedPacketComponent>(
        &'a mut self,
        component: &'a P,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>>;
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

    #[cfg(feature = "encryption")]
    fn no_crypt_wrap(self) -> EncryptedWriter<Self>
    where
        Self: Sized,
    {
        EncryptedWriter::noop(self)
    }

    #[cfg(feature = "encryption")]
    fn encrypt_stream(self, encryption: Encryption) -> EncryptedWriter<Self>
    where
        Self: Sized,
    {
        EncryptedWriter::new(self, encryption)
    }

    fn encode_packet<'a, P: OwnedPacketComponent>(
        &'a mut self,
        component: &'a P,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        P::encode_owned(component, self)
    }
}

pub mod var_num {
    use crate::{err, err_explain};
    use pin_project_lite::pin_project;
    use std::future::Future;
    use std::marker::PhantomPinned;
    use std::pin::Pin;
    use std::task::{ready, Context, Poll};
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

    macro_rules! declare_var_num_ext {
        (
        $typing:ty,
        $sub_typing:ty,
        $size_fn:ident,
        $read_fn:ident,
        $read_struct:ident,
        $write_fn:ident,
        $write_struct:ident,
        $bit_limit:literal,
        $and_check:literal
    ) => {
            pub fn $size_fn(var_num: $typing) -> usize {
                let mut temp: $sub_typing = var_num as $sub_typing;
                let mut size = 0;
                loop {
                    if (temp & $and_check) == 0 {
                        return size + 1;
                    }
                    size += 1;
                    temp = temp.overflowing_shr(7).0;
                }
            }

            pub(crate) fn $read_fn<A>(reader: &mut A) -> $read_struct<A>
            where
                A: AsyncRead + Unpin + ?Sized,
            {
                $read_struct {
                    reader,
                    value: 0,
                    bit_offset: 0,
                    _pin: PhantomPinned,
                }
            }

            pin_project! {
                #[derive(Debug)]
                #[must_use = "futures do nothing unless you `.await` or poll them"]
                pub struct $read_struct<'a, A: ?Sized> {
                    reader: &'a mut A,
                    value: $typing,
                    bit_offset: u32,
                    // Make this future `!Unpin` for compatibility with async trait methods.
                    #[pin]
                    _pin: PhantomPinned,
                }
            }

            impl<A> Future for $read_struct<'_, A>
            where
                A: AsyncRead + Unpin + ?Sized,
            {
                type Output = crate::transport::Result<$typing>;

                fn poll(
                    self: Pin<&mut Self>,
                    cx: &mut Context<'_>,
                ) -> Poll<crate::transport::Result<$typing>> {
                    let me = self.project();

                    loop {
                        if *me.bit_offset >= $bit_limit {
                            return Poll::Ready(Err(err_explain!("VarInt too large")));
                        };

                        let mut inner = [0u8; 1];
                        let mut buf = ReadBuf::new(inner.as_mut());
                        ready!(Pin::new(&mut *me.reader).poll_read(cx, &mut buf))?;
                        if buf.filled().len() == 0 {
                            return Poll::Ready(Err(err!(crate::prelude::ErrorType::EOF)));
                        }
                        let byte = buf.filled()[0];
                        *me.value |= <$typing>::from(byte & 0b0111_1111)
                            .overflowing_shl(*me.bit_offset)
                            .0;
                        *me.bit_offset += 7;
                        if byte & 0b1000_0000 == 0 {
                            return Poll::Ready(Ok(*me.value));
                        }
                    }
                }
            }

            pub(crate) fn $write_fn<A>(writer: &mut A, value: $typing) -> $write_struct<A>
            where
                A: AsyncWrite + Unpin + ?Sized,
            {
                $write_struct {
                    writer,
                    value,
                    _pin: PhantomPinned,
                }
            }

            pin_project! {
                #[derive(Debug)]
                #[must_use = "futures do nothing unless you `.await` or poll them"]
                pub struct $write_struct<'a, A: ?Sized> {
                    writer: &'a mut A,
                    value: $typing,
                    // Make this future `!Unpin` for compatibility with async trait methods.
                    #[pin]
                    _pin: PhantomPinned,
                }
            }

            impl<A> Future for $write_struct<'_, A>
            where
                A: AsyncWrite + Unpin + ?Sized,
            {
                type Output = crate::transport::Result<()>;

                fn poll(
                    self: Pin<&mut Self>,
                    cx: &mut Context<'_>,
                ) -> Poll<crate::transport::Result<()>> {
                    let me = self.project();

                    let mut value: $sub_typing = *me.value as $sub_typing;
                    loop {
                        if (value & $and_check) == 0 {
                            ready!(Pin::new(&mut *me.writer).poll_write(cx, &[value as u8]))?;
                            return Poll::Ready(Ok(()));
                        }
                        ready!(Pin::new(&mut *me.writer)
                            .poll_write(cx, &[(value & 0x7F | 0x80) as u8]))?;
                        value = value.overflowing_shr(7).0;
                        *me.value = value.try_into().unwrap();
                    }
                }
            }
        };
    }

    declare_var_num_ext!(
        i32,
        u32,
        size_var_int,
        read_var_int,
        ReadVarInt,
        write_var_int,
        WriteVarInt,
        35,
        0xFFFFFF80u32
    );

    declare_var_num_ext!(
        i64,
        u64,
        size_var_long,
        read_var_long,
        ReadVarLong,
        write_var_long,
        WriteVarLong,
        70,
        0xFFFFFFFFFFFFFF80u64
    );
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
