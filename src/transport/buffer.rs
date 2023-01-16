use tokio::io::{AsyncRead, AsyncWrite};

use crate::prelude::PacketComponent;
use crate::transport::buffer::var_num::{ReadVarInt, ReadVarLong, WriteVarInt, WriteVarLong};
use crate::transport::encryption::{Cipher, CipherAttachedReader};
use crate::PinnedLivelyResult;

pub trait DraxReadExt {
    fn read_var_int(&mut self) -> ReadVarInt<'_, Self>;

    fn read_var_long(&mut self) -> ReadVarLong<'_, Self>;

    fn decode_component<'a, C: Send + Sync, P: PacketComponent<C>>(
        &'a mut self,
        context: &'a mut C,
    ) -> PinnedLivelyResult<'a, P::ComponentType>
    where
        P: Sized;

    fn decrypt<'a>(&'a mut self, cipher: &'a mut Cipher) -> CipherAttachedReader<'a, Self>
    where
        Self: Sized;
}

impl<T> DraxReadExt for T
where
    T: AsyncRead + Unpin + Send + Sync + ?Sized,
{
    fn read_var_int(&mut self) -> ReadVarInt<'_, Self> {
        var_num::read_var_int(self)
    }

    fn read_var_long(&mut self) -> ReadVarLong<'_, Self> {
        var_num::read_var_long(self)
    }

    fn decode_component<'a, C: Send + Sync, P: PacketComponent<C>>(
        &'a mut self,
        context: &'a mut C,
    ) -> PinnedLivelyResult<'a, P::ComponentType>
    where
        P: Sized,
    {
        P::decode(context, self)
    }

    fn decrypt<'a>(&'a mut self, cipher: &'a mut Cipher) -> CipherAttachedReader<'a, Self>
    where
        Self: Sized,
    {
        CipherAttachedReader {
            inner: self,
            cipher,
        }
    }
}

pub trait DraxWriteExt {
    fn write_var_int(&mut self, value: i32) -> WriteVarInt<'_, Self>;

    fn write_var_long(&mut self, value: i64) -> WriteVarLong<'_, Self>;

    fn encode_component<'a, C: Send + Sync, P: PacketComponent<C>>(
        &'a mut self,
        context: &'a mut C,
        component: &'a P::ComponentType,
    ) -> PinnedLivelyResult<'a, ()>;
}

impl<T> DraxWriteExt for T
where
    T: AsyncWrite + Unpin + Send + Sync + ?Sized,
{
    fn write_var_int(&mut self, value: i32) -> WriteVarInt<'_, Self> {
        var_num::write_var_int(self, value)
    }

    fn write_var_long(&mut self, value: i64) -> WriteVarLong<'_, Self> {
        var_num::write_var_long(self, value)
    }

    fn encode_component<'a, C: Send + Sync, P: PacketComponent<C>>(
        &'a mut self,
        context: &'a mut C,
        component: &'a P::ComponentType,
    ) -> PinnedLivelyResult<'a, ()> {
        P::encode(component, context, self)
    }
}

pub mod var_num {
    use std::future::Future;
    use std::marker::PhantomPinned;
    use std::pin::Pin;
    use std::task::{ready, Context, Poll};

    use pin_project_lite::pin_project;
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

    use crate::{err, err_explain};

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
    use std::io::Cursor;

    use super::{DraxReadExt, DraxWriteExt};

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
