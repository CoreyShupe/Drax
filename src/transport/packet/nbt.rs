// use std::future::Future;
// use std::pin::Pin;
//
// use tokio::io::{AsyncRead, AsyncWrite};
//
// use crate::nbt::{read_nbt, size_nbt, write_optional_nbt, CompoundTag};
// use crate::prelude::PacketComponent;
// use crate::transport::packet::Size;
//
// impl<C> PacketComponent<C> for Option<CompoundTag> {
//     type ComponentType = Self;
//
//     fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
//         _: &'a mut C,
//         read: &'a mut A,
//     ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
//     where
//         Self: Sized,
//     {
//         Box::pin(read_nbt(read, 0x200000u64))
//     }
//
//     fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
//         component_ref: &'a Self,
//         _: &'a mut C,
//         write: &'a mut A,
//     ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
//         Box::pin(write_optional_nbt(component_ref, write))
//     }
//
//     fn size(component_ref: &Self, _: &mut C) -> crate::prelude::Result<Size> {
//         Ok(Size::Dynamic(
//             component_ref
//                 .as_ref()
//                 .map(|ctag| size_nbt(ctag))
//                 .unwrap_or(1),
//         ))
//     }
// }
