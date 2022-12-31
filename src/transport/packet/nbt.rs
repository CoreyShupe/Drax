use std::future::Future;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::nbt::{read_nbt, size_nbt, write_optional_nbt, CompoundTag};
use crate::transport::packet::{LimitedPacketComponent, OwnedPacketComponent, Size};

impl OwnedPacketComponent for Option<CompoundTag> {
    fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
    where
        Self: Sized,
    {
        Box::pin(read_nbt(read, 0x200000u64))
    }

    fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
        &'a self,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(write_optional_nbt(self, write))
    }

    fn size_owned(&self) -> Size {
        Size::Dynamic(self.as_ref().map(|ctag| size_nbt(ctag)).unwrap_or(1))
    }
}

impl LimitedPacketComponent<u64> for Option<CompoundTag> {
    fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
        limit: Option<u64>,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
    where
        Self: Sized,
        u64: 'a,
    {
        Box::pin(read_nbt(read, limit.unwrap_or(0x200000u64)))
    }
}
