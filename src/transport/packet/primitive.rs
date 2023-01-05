use std::future::Future;
use std::mem::size_of;
use std::pin::Pin;

use crate::transport::buffer::var_num::{size_var_int, size_var_long};
use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

use super::{PacketComponent, Size};

macro_rules! define_primitive_bind {
    ($($prim:ty),*) => {
        $(
            impl<C> PacketComponent<C> for $prim {
                type ComponentType = $prim;
                fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
                    _: &'a mut C,
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
                fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
                    component_ref: &'a Self,
                    _: &'a mut C,
                    write: &'a mut A,
                ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
                    Box::pin(async move {
                        write.write_all(component_ref.to_be_bytes().as_ref()).await?;
                        Ok(())
                    })
                }
                fn size(_: &Self, __: &mut C) -> crate::prelude::Result<Size> {
                    Ok(Size::Constant(size_of::<Self>()))
                }
            }
        )*
    }
}

define_primitive_bind!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);

impl<C> PacketComponent<C> for bool {
    type ComponentType = bool;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move {
            let b = read.read_u8().await?;
            Ok(b != 0x0)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        _: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write
                .write_u8(if *component_ref { 0x1 } else { 0x0 })
                .await?;
            Ok(())
        })
    }

    fn size(_: &Self::ComponentType, _: &mut C) -> crate::prelude::Result<Size> {
        Ok(Size::Constant(1))
    }
}

pub struct VarInt;

impl<C> PacketComponent<C> for VarInt {
    type ComponentType = i32;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move { read.read_var_int().await })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        _: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move { write.write_var_int(*component_ref).await })
    }

    fn size(input: &Self::ComponentType, _: &mut C) -> crate::prelude::Result<Size> {
        Ok(Size::Dynamic(size_var_int(*input)))
    }
}

pub struct VarLong;

impl<C> PacketComponent<C> for VarLong {
    type ComponentType = i64;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move { read.read_var_long().await })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        _: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move { write.write_var_long(*component_ref).await })
    }

    fn size(input: &Self::ComponentType, _: &mut C) -> crate::prelude::Result<Size> {
        Ok(Size::Dynamic(size_var_long(*input)))
    }
}

impl<C> PacketComponent<C> for Uuid {
    type ComponentType = Uuid;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move {
            let mut buf = [0; 16];
            read.read_exact(&mut buf).await?;
            let uuid = Uuid::from_slice(&buf)?;
            Ok(uuid)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        _: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_all(component_ref.as_bytes()).await?;
            Ok(())
        })
    }

    fn size(_: &Self::ComponentType, _: &mut C) -> crate::prelude::Result<Size> {
        Ok(Size::Constant(size_of::<u64>() * 2))
    }
}
