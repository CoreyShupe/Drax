use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::transport::packet::vec::VecU8;
use crate::transport::packet::{PacketComponent, Size};

pub struct JsonDelegate<T> {
    _phantom_t: PhantomData<T>,
}

impl<C, T> PacketComponent<C> for JsonDelegate<T>
where
    T: for<'de> Deserialize<'de>,
    T: Serialize,
{
    type ComponentType = T;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        context: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let bytes = VecU8::decode(context, read).await?;
            let value: T = serde_json::from_slice(&bytes)?;
            Ok(value)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        context: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            let bytes = serde_json::to_vec(&component_ref)?;
            VecU8::encode(&bytes, context, write).await
        })
    }

    fn size(input: &Self::ComponentType, context: &mut C) -> crate::prelude::Result<Size> {
        VecU8::size(&serde_json::to_vec(&input)?, context)
    }
}
