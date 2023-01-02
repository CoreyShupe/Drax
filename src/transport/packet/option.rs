use crate::transport::packet::{PacketComponent, Size};
use std::future::Future;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub struct Maybe<T> {
    _phantom_t: T,
}

impl<C, T> PacketComponent<C> for Maybe<T>
where
    T: PacketComponent<C>,
{
    type ComponentType = Option<T::ComponentType>;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        context: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move {
            let has_value = read.read_u8().await?;
            if has_value != 0x0 {
                Ok(Some(T::decode(context, read).await?))
            } else {
                Ok(None)
            }
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        context: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write
                .write_u8(if component_ref.is_some() { 1 } else { 0 })
                .await?;
            if let Some(value) = component_ref {
                T::encode(value, context, write).await?;
            }
            Ok(())
        })
    }

    fn size(input: &Self::ComponentType, context: &mut C) -> crate::prelude::Result<Size> {
        Ok(if let Some(value) = input {
            match T::size(value, context)? {
                Size::Dynamic(x) | Size::Constant(x) => Size::Dynamic(x + 1),
            }
        } else {
            Size::Dynamic(1)
        })
    }
}
