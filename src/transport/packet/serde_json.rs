use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::transport::packet::{OwnedPacketComponent, PacketComponent, Size};

pub struct JsonDelegate<T> {
    _phantom_t: PhantomData<T>,
}

impl<T> PacketComponent for JsonDelegate<T>
where
    T: for<'de> Deserialize<'de>,
    T: Serialize,
{
    type ComponentType = T;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let bytes = Vec::<u8>::decode_owned(read).await?;
            let value: T = serde_json::from_slice(&bytes)?;
            Ok(value)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            let bytes = serde_json::to_vec(&component_ref)?;
            bytes.encode_owned(write).await
        })
    }

    fn size(input: &Self::ComponentType) -> Size {
        // todo remove panic
        serde_json::to_vec(&input).unwrap().size_owned()
    }
}
