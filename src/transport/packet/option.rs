use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::transport::packet::{PacketComponent, Size};
use crate::PinnedLivelyResult;

pub struct Maybe<T> {
    _phantom_t: T,
}

impl<C: Send + Sync, T> PacketComponent<C> for Maybe<T>
where
    T: PacketComponent<C>,
{
    type ComponentType = Option<T::ComponentType>;

    fn decode<'a, A: AsyncRead + Unpin + Send + Sync + ?Sized>(
        context: &'a mut C,
        read: &'a mut A,
    ) -> PinnedLivelyResult<'a, Self::ComponentType> {
        Box::pin(async move {
            let has_value = read.read_u8().await?;
            if has_value != 0x0 {
                Ok(Some(T::decode(context, read).await?))
            } else {
                Ok(None)
            }
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + Send + Sync + ?Sized>(
        component_ref: &'a Self::ComponentType,
        context: &'a mut C,
        write: &'a mut A,
    ) -> PinnedLivelyResult<'a, ()> {
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
