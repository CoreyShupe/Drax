use std::future::Future;
use std::mem::size_of;
use std::pin::Pin;

use crate::transport::buffer::var_num::{size_var_int, size_var_long};
use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
use crate::transport::packet::PacketComponent;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::{OwnedPacketComponent, Size};

macro_rules! define_primitive_bind {
        ($($prim:ty),*) => {
            $(
                impl OwnedPacketComponent for $prim {
                    fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
                        read: &'a mut A,
                    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
                    where
                        Self: Sized,
                    {
                        Box::pin(async move {
                            let mut buf = [0; size_of::<Self>()];
                            read.read_exact(&mut buf).await?;
                            Ok(Self::from_be_bytes(buf))
                        })
                    }

                    fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
                        &'a self,
                        write: &'a mut A,
                    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
                        Box::pin(async move {
                            write.write_all(self.to_be_bytes().as_ref()).await?;
                            Ok(())
                        })
                    }

                    fn size_owned(&self) -> Size {
                        Size::Constant(size_of::<Self>())
                    }
                }
            )*
        }
    }

define_primitive_bind!(u16, u32, u64, i8, i16, i32, i64, f32, f64);

pub struct B1;

impl PacketComponent for B1 {
    type ComponentType = u8;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move {
            let res = read.read_u8().await?;
            Ok(res)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_u8(*component_ref).await?;
            Ok(())
        })
    }

    fn size(_: &Self::ComponentType) -> Size {
        Size::Constant(1)
    }
}

pub struct VarInt;

impl PacketComponent for VarInt {
    type ComponentType = i32;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move { read.read_var_int().await })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move { write.write_var_int(*component_ref).await })
    }

    fn size(input: &Self::ComponentType) -> Size {
        Size::Dynamic(size_var_int(*input))
    }
}

pub struct VarLong;

impl PacketComponent for VarLong {
    type ComponentType = i64;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move { read.read_var_long().await })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move { write.write_var_long(*component_ref).await })
    }

    fn size(input: &Self::ComponentType) -> Size {
        Size::Dynamic(size_var_long(*input))
    }
}
