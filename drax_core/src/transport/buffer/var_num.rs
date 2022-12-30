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
                        return Poll::Ready(Err(err!(crate::ErrorType::EOF)));
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
                    ready!(
                        Pin::new(&mut *me.writer).poll_write(cx, &[(value & 0x7F | 0x80) as u8])
                    )?;
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
