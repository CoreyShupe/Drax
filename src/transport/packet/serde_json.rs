use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::transport::packet::vec::VecU8;
use crate::transport::packet::{PacketComponent, Size};
use crate::PinnedLivelyResult;

pub struct JsonDelegate<T> {
    _phantom_t: PhantomData<T>,
}

impl<C: Send + Sync, T> PacketComponent<C> for JsonDelegate<T>
where
    T: for<'de> Deserialize<'de>,
    T: Serialize + Send + Sync,
{
    type ComponentType = T;

    fn decode<'a, A: AsyncRead + Unpin + Send + Sync + ?Sized>(
        context: &'a mut C,
        read: &'a mut A,
    ) -> PinnedLivelyResult<'a, Self::ComponentType>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let bytes = VecU8::decode(context, read).await?;
            let value: T = serde_json::from_slice(&bytes)?;
            Ok(value)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + Send + Sync + ?Sized>(
        component_ref: &'a Self::ComponentType,
        context: &'a mut C,
        write: &'a mut A,
    ) -> PinnedLivelyResult<'a, ()> {
        Box::pin(async move {
            let bytes = serde_json::to_vec(&component_ref)?;
            VecU8::encode(&bytes, context, write).await
        })
    }

    fn size(input: &Self::ComponentType, context: &mut C) -> crate::prelude::Result<Size> {
        VecU8::size(&serde_json::to_vec(&input)?, context)
    }
}
