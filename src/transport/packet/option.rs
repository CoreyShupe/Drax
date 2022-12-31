use crate::transport::packet::{
    LimitedPacketComponent, OwnedPacketComponent, PacketComponent, Size,
};
use std::future::Future;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub struct Maybe<T> {
    _phantom_t: T,
}

impl<T> PacketComponent for Maybe<T>
where
    T: OwnedPacketComponent,
{
    type ComponentType = Option<T>;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move {
            let has_value = read.read_u8().await?;
            if has_value != 0x0 {
                Ok(Some(T::decode_owned(read).await?))
            } else {
                Ok(None)
            }
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write
                .write_u8(if component_ref.is_some() { 1 } else { 0 })
                .await?;
            if let Some(value) = component_ref {
                value.encode_owned(write).await?;
            }
            Ok(())
        })
    }

    fn size(input: &Self::ComponentType) -> Size {
        if let Some(value) = input {
            match value.size_owned() {
                Size::Dynamic(x) | Size::Constant(x) => Size::Dynamic(x + 1),
            }
        } else {
            Size::Dynamic(1)
        }
    }
}

impl<T, L> LimitedPacketComponent<L> for Maybe<T>
where
    T: OwnedPacketComponent + PacketComponent<ComponentType = T>,
    T: LimitedPacketComponent<L>,
{
    fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
        limit: Option<L>,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
    where
        L: 'a,
    {
        Box::pin(async move {
            let has_value = read.read_u8().await?;
            if has_value != 0x0 {
                Ok(Some(T::decode_with_limit(read, limit).await?))
            } else {
                Ok(None)
            }
        })
    }
}

pub struct MaybeDelegate<T> {
    _phantom_t: T,
}

impl<T> PacketComponent for MaybeDelegate<T>
where
    T: PacketComponent,
{
    type ComponentType = Option<T::ComponentType>;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move {
            let has_value = read.read_u8().await?;
            if has_value != 0x0 {
                Ok(Some(T::decode(read).await?))
            } else {
                Ok(None)
            }
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write
                .write_u8(if component_ref.is_some() { 1 } else { 0 })
                .await?;
            if let Some(value) = component_ref {
                T::encode(value, write).await?;
            }
            Ok(())
        })
    }

    fn size(input: &Self::ComponentType) -> Size {
        if let Some(value) = input {
            match T::size(value) {
                Size::Dynamic(x) | Size::Constant(x) => Size::Dynamic(x + 1),
            }
        } else {
            Size::Dynamic(1)
        }
    }
}
