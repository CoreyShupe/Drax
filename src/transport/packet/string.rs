use std::future::Future;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::throw_explain;
use crate::transport::buffer::var_num::size_var_int;
use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
use crate::transport::packet::{PacketComponent, Size};

const STRING_DEFAULT_CAP: i32 = 32767 * 4;

impl<C> PacketComponent<C> for String {
    type ComponentType = Self;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let len = read.read_var_int().await?;
            if len > STRING_DEFAULT_CAP {
                throw_explain!(format!(
                    "String exceeded length bound {STRING_DEFAULT_CAP}"
                ))
            }
            let mut buf = vec![0; len as usize];
            read.read_exact(&mut buf).await?;
            Ok(String::from_utf8(buf)?)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self,
        _: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_var_int(component_ref.len() as i32).await?;
            write.write_all(component_ref.as_bytes()).await?;
            Ok(())
        })
    }

    fn size(component_ref: &Self, _: &mut C) -> crate::prelude::Result<Size> {
        Ok(Size::Dynamic(
            component_ref.len() + size_var_int(component_ref.len() as i32),
        ))
    }
}

pub struct LimitedString<const N: usize>;

impl<C, const N: usize> PacketComponent<C> for LimitedString<N> {
    type ComponentType = String;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move {
            let string_size = read.read_var_int().await?;
            if string_size > N as i32 * 4 {
                throw_explain!(format!(
                    "While encoding; string exceeded length bound {}",
                    N * 4
                ))
            }

            let mut buf = vec![0; string_size as usize];
            read.read_exact(&mut buf).await?;
            Ok(String::from_utf8(buf)?)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        context: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        if component_ref.len() > N * 4 {
            return Box::pin(async move {
                throw_explain!(format!(
                    "While decoding; string exceeded length bound {}",
                    N * 4
                ))
            });
        }

        String::encode(component_ref, context, write)
    }

    fn size(input: &Self::ComponentType, context: &mut C) -> crate::prelude::Result<Size> {
        String::size(input, context)
    }
}
