use std::future::Future;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Size {
    Dynamic(usize),
    Constant(usize),
}

impl std::ops::Add for Size {
    type Output = Size;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Size::Dynamic(x), Size::Dynamic(y))
            | (Size::Dynamic(x), Size::Constant(y))
            | (Size::Constant(x), Size::Dynamic(y)) => Size::Dynamic(x + y),
            (Size::Constant(x), Size::Constant(y)) => Size::Constant(x + y),
        }
    }
}

impl std::ops::Add<usize> for Size {
    type Output = Size;

    fn add(self, rhs: usize) -> Self::Output {
        match self {
            Size::Dynamic(x) | Size::Constant(x) => Size::Dynamic(x + rhs),
        }
    }
}

/// Defines a trait extension for `AsyncWrite` which allows quick encoding of packet components.
/// This will likely be used as a `Cursor` extension for buffering packets for writing.
pub trait PacketEncoder {
    fn encode_packet<'a, T: PacketComponent<ComponentType = T>>(
        &'a mut self,
        component: &'a T,
    ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>>;
}

impl<A> PacketEncoder for A
where
    A: AsyncWrite + Unpin,
{
    fn encode_packet<'a, T: PacketComponent<ComponentType = T>>(
        &'a mut self,
        component: &'a T,
    ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
        T::encode(component, self)
    }
}

/// Defines a trait extension for `AsyncRead` which allows quick decoding of packet components.
pub trait PacketDecoder {
    fn decode_packet<'a, T: PacketComponent<ComponentType = T>>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::Result<T>> + 'a>>
    where
        T: Sized;
}

impl<A> PacketDecoder for A
where
    A: AsyncRead + Unpin,
{
    fn decode_packet<'a, T: PacketComponent<ComponentType = T>>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::Result<T>> + 'a>>
    where
        T: Sized,
    {
        T::decode(self)
    }
}

/// Defines a structure that can be encoded and decoded.
pub trait PacketComponent {
    type ComponentType: Sized;

    /// Decodes the packet component from the given reader.
    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Self::ComponentType>> + 'a>>;

    /// Encodes the packet component to the given writer.
    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>>;

    fn size(input: &Self::ComponentType) -> Size;
}

/// Declares a packet component which resolves itself.
pub trait OwnedPacketComponent {
    /// Decodes the packet component from the given reader.
    fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>;

    /// Encodes the packet component to the given writer.
    fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
        &'a self,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>>;

    fn size_owned(&self) -> Size;
}

impl<T> PacketComponent for T
where
    T: OwnedPacketComponent,
{
    type ComponentType = T;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Self::ComponentType>> + 'a>> {
        T::decode_owned(read)
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
        T::encode_owned(component_ref, write)
    }

    fn size(input: &Self::ComponentType) -> Size {
        T::size_owned(input)
    }
}

/// A trait defining a packet component which is limited in size.
///
/// # Parameters
///
/// * `Limit` - The type which the limit should be defined as.
pub trait LimitedPacketComponent<Limit>: PacketComponent {
    /// Decodes the packet component from the given reader.
    ///
    /// # Parameters
    ///
    /// * `read` - The reader to read from.
    /// * `limit` - The maximum size of the packet component.
    fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
        limit: Option<Limit>,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Self::ComponentType>> + 'a>>
    where
        Limit: 'a;
}

#[cfg(feature = "nbt")]
pub mod nbt {
    use std::future::Future;
    use std::pin::Pin;

    use tokio::io::{AsyncRead, AsyncWrite};

    use crate::nbt::{read_nbt, size_nbt, write_optional_nbt, CompoundTag};
    use crate::transport::packet::{LimitedPacketComponent, OwnedPacketComponent, Size};

    impl OwnedPacketComponent for Option<CompoundTag> {
        fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(read_nbt(read, 0x200000u64))
        }

        fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
            &'a self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(write_optional_nbt(self, write))
        }

        fn size_owned(&self) -> Size {
            Size::Dynamic(input.as_ref().map(|ctag| size_nbt(ctag)).unwrap_or(1))
        }
    }

    impl LimitedPacketComponent<u64> for Option<CompoundTag> {
        fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
            limit: Option<u64>,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
            u64: 'a,
        {
            Box::pin(read_nbt(read, limit.unwrap_or(0x200000u64)))
        }
    }
}

pub mod primitive {
    use std::future::Future;
    use std::mem::size_of;
    use std::pin::Pin;

    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

    use super::{OwnedPacketComponent, Size};

    macro_rules! define_primitive_bind {
        ($($prim:ty),*) => {
            $(
                impl OwnedPacketComponent for $prim {
                    fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
                        read: &'a mut A,
                    ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
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
                    ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
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
}

#[cfg(feature = "serde")]
pub mod serde_json {
    use std::future::Future;
    use std::pin::Pin;

    use serde::{Deserialize, Serialize};
    use tokio::io::{AsyncRead, AsyncWrite};

    use crate::transport::packet::{OwnedPacketComponent, PacketComponent, Size};

    pub struct JsonWrapper<T> {
        value: T,
    }

    impl<T> JsonWrapper<T> {
        pub fn wrap(value: T) -> Self {
            Self { value }
        }
    }

    impl<T> From<T> for JsonWrapper<T> {
        fn from(value: T) -> Self {
            Self { value }
        }
    }

    impl<T> OwnedPacketComponent for JsonWrapper<T>
    where
        T: for<'de> Deserialize<'de>,
        T: Serialize,
    {
        fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(async move {
                let bytes = Vec::<u8>::decode(read).await?;
                let value: T = serde_json::from_slice(&bytes)?;
                Ok(value.into())
            })
        }

        fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
            &'a self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                let bytes = serde_json::to_vec(&self.value)?;
                Vec::<u8>::encode(&bytes, write).await
            })
        }

        fn size_owned(&self) -> Size {
            let bytes = serde_json::to_vec(&self.value).unwrap();
            Vec::<u8>::size(&bytes)
        }
    }
}

pub mod vec {
    use std::future::Future;
    use std::mem::MaybeUninit;
    use std::ops::Deref;
    use std::pin::Pin;

    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

    use crate::transport::buffer::var_num::size_var_int;
    use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
    use crate::transport::packet::{LimitedPacketComponent, PacketComponent, Size};

    pub struct ByteDrain {
        bytes: Vec<u8>,
    }

    impl ByteDrain {
        pub fn into_inner(self) -> Vec<u8> {
            self.bytes
        }
    }

    impl From<Vec<u8>> for ByteDrain {
        fn from(value: Vec<u8>) -> Self {
            Self { bytes: value }
        }
    }

    impl Deref for ByteDrain {
        type Target = Vec<u8>;

        fn deref(&self) -> &Self::Target {
            &self.bytes
        }
    }

    impl PacketComponent for ByteDrain {
        type ComponentType = Self;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(async move {
                let mut bytes = vec![];
                read.read_to_end(&mut bytes).await?;
                Ok(bytes.into())
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                write.write_all(&component_ref.bytes).await?;
                Ok(())
            })
        }

        fn size(input: &Self) -> Size {
            Size::Dynamic(input.len())
        }
    }

    impl<const N: usize> PacketComponent for [u8; N] {
        type ComponentType = Self;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(async move {
                let mut buf = [0; N];
                read.read_exact(&mut buf).await?;
                Ok(buf)
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                write.write_all(component_ref).await?;
                Ok(())
            })
        }

        fn size(_: &Self) -> Size {
            Size::Constant(N)
        }
    }

    impl<T, const N: usize> PacketComponent for [T; N]
    where
        T: PacketComponent<ComponentType = T>,
    {
        type ComponentType = Self;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(async move {
                let mut arr: [MaybeUninit<T>; N] = MaybeUninit::uninit_array();
                for i in 0..N {
                    arr[i] = MaybeUninit::new(T::decode(read).await?);
                }
                Ok(arr.map(|x| unsafe { x.assume_init() }))
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                for x in component_ref {
                    T::encode(x, write).await?;
                }
                Ok(())
            })
        }

        fn size(input: &Self) -> Size {
            let mut dynamic_counter = 0;
            for item in input {
                match item.size() {
                    Size::Constant(x) => return Size::Constant(x * N),
                    Size::Dynamic(x) => dynamic_counter += x,
                }
            }
            Size::Dynamic(dynamic_counter)
        }
    }

    impl<T, L, const N: usize> LimitedPacketComponent<L> for [T; N]
    where
        T: LimitedPacketComponent<L, ComponentType = T>,
        L: Copy,
    {
        fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
            limit: Option<L>,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
            L: 'a,
        {
            Box::pin(async move {
                let mut arr: [MaybeUninit<T>; N] = MaybeUninit::uninit_array();
                for i in 0..N {
                    arr[i] = MaybeUninit::new(T::decode_with_limit(read, limit).await?);
                }
                Ok(arr.map(|x| unsafe { x.assume_init() }))
            })
        }
    }

    impl PacketComponent for Vec<u8> {
        type ComponentType = Self;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(async move {
                let len = read.read_var_int().await?;
                let mut buf = vec![0u8; len as usize];
                read.read_exact(&mut buf).await?;
                Ok(buf)
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                write.write_var_int(component_ref.len() as i32).await?;
                write.write_all(component_ref).await?;
                Ok(())
            })
        }

        fn size(input: &Self::ComponentType) -> Size {
            Size::Dynamic(input.len() + size_var_int(input.len() as i32))
        }
    }

    impl<T> PacketComponent for Vec<T>
    where
        T: PacketComponent<ComponentType = T>,
    {
        type ComponentType = Self;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(async move {
                let len = read.read_var_int().await?;
                let mut vec = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    vec.push(T::decode(read).await?);
                }
                Ok(vec)
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                write.write_var_int(component_ref.len() as i32).await?;
                for item in component_ref {
                    item.encode(write).await?;
                }
                Ok(())
            })
        }

        fn size(input: &Self::ComponentType) -> Size {
            let var_int_size = size_var_int(input.len() as i32);
            let mut dynamic_counter = var_int_size;
            for item in input {
                match item.size() {
                    Size::Constant(x) => return Size::Dynamic((x * input.len()) + var_int_size),
                    Size::Dynamic(x) => dynamic_counter += x,
                }
            }
            Size::Dynamic(dynamic_counter)
        }
    }

    impl<T, N> LimitedPacketComponent<N> for Vec<T>
    where
        T: LimitedPacketComponent<N, ComponentType = T>,
        N: Copy,
    {
        fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
            limit: Option<N>,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
            N: 'a,
        {
            Box::pin(async move {
                let len = read.read_var_int().await?;
                let mut vec = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    vec.push(T::decode_with_limit(read, limit).await?);
                }
                Ok(vec)
            })
        }
    }
}

pub mod string {
    use std::future::Future;
    use std::pin::Pin;

    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

    use crate::throw_explain;
    use crate::transport::buffer::var_num::size_var_int;
    use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
    use crate::transport::packet::{LimitedPacketComponent, PacketComponent, Size};

    const STRING_DEFAULT_CAP: i32 = 32767 * 4;

    impl PacketComponent for String {
        type ComponentType = Self;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(async move {
                let len = read.read_var_int().await?;
                if len > STRING_DEFAULT_CAP {
                    throw_explain!(format!(
                        "String exceeded length bound {}",
                        STRING_DEFAULT_CAP
                    ))
                }
                let mut buf = vec![0; len as usize];
                read.read_exact(&mut buf).await?;
                Ok(String::from_utf8(buf)?)
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                write.write_var_int(component_ref.len() as i32).await?;
                write.write_all(component_ref.as_bytes()).await?;
                Ok(())
            })
        }

        fn size(input: &Self) -> Size {
            Size::Dynamic(input.len() + size_var_int(input.len() as i32))
        }
    }

    impl LimitedPacketComponent<i32> for String {
        fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
            limit: Option<i32>,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
            i32: 'a,
        {
            Box::pin(async move {
                let len = read.read_var_int().await?;
                if let Some(limit) = limit {
                    let limit = limit * 4;
                    if len > limit {
                        throw_explain!(format!("String exceeded length bound {}", limit))
                    }
                } else if len > STRING_DEFAULT_CAP {
                    throw_explain!(format!(
                        "String exceeded length bound {}",
                        STRING_DEFAULT_CAP
                    ))
                }
                let mut buf = vec![0; len as usize];
                read.read_exact(&mut buf).await?;
                Ok(String::from_utf8(buf)?)
            })
        }
    }
}

pub mod option {
    use std::future::Future;
    use std::ops::Deref;
    use std::pin::Pin;

    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

    use crate::transport::packet::{LimitedPacketComponent, PacketComponent, Size};

    /// Clone of the `Option` type used for serialization and deserialization.
    /// This type denotes that there will be a boolean header before the value.
    pub struct Maybe<T> {
        /// The value of the option.
        inner: Option<T>,
    }

    impl<T> From<Option<T>> for Maybe<T> {
        fn from(inner: Option<T>) -> Self {
            Self { inner }
        }
    }

    impl<T> Into<Option<T>> for Maybe<T> {
        fn into(self) -> Option<T> {
            self.inner
        }
    }

    impl<T> Deref for Maybe<T> {
        type Target = Option<T>;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl PacketComponent for Maybe<u8> {
        type ComponentType = Self;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(async move {
                let has_value = read.read_u8().await?;
                if has_value == 0 {
                    Ok(Maybe { inner: None })
                } else {
                    Ok(Maybe {
                        inner: Some(read.read_u8().await?),
                    })
                }
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                if let Some(value) = &component_ref.inner {
                    write.write_u8(1).await?;
                    write.write_u8(*value).await?;
                } else {
                    write.write_u8(0).await?;
                }
                Ok(())
            })
        }

        fn size(input: &Self::ComponentType) -> Size {
            Size::Dynamic(1 + input.inner.map(|_| 1).unwrap_or(0))
        }
    }

    impl<T> PacketComponent for Maybe<T>
    where
        T: PacketComponent<ComponentType = T>,
    {
        type ComponentType = Self;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(async move {
                let has_value = read.read_u8().await?;
                if has_value == 0 {
                    Ok(Maybe { inner: None })
                } else {
                    let value = T::decode(read).await?;
                    Ok(Maybe { inner: Some(value) })
                }
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                if let Some(value) = &component_ref.inner {
                    write.write_u8(1).await?;
                    value.encode(write).await?;
                } else {
                    write.write_u8(0).await?;
                }
                Ok(())
            })
        }

        fn size(input: &Self::ComponentType) -> Size {
            match input {
                Maybe { inner: Some(value) } => Size::Dynamic(1 + value.size()),
                Maybe { inner: None } => Size::Dynamic(1),
            }
            Size::Dynamic(1 + input.inner.as_ref().map(|v| v.size()).unwrap_or(0))
        }
    }

    impl<T, N> LimitedPacketComponent<N> for Maybe<T>
    where
        T: LimitedPacketComponent<N, ComponentType = T>,
    {
        fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
            limit: Option<N>,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
            N: 'a,
        {
            Box::pin(async move {
                let has_value = read.read_u8().await?;
                if has_value == 0 {
                    Ok(Maybe { inner: None })
                } else {
                    let value = T::decode_with_limit(read, limit).await?;
                    Ok(Maybe { inner: Some(value) })
                }
            })
        }
    }
}

#[cfg(test)]
mod test {
    use std::future::Future;
    use std::io::Cursor;
    use std::mem::size_of;
    use std::pin::Pin;

    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

    use crate::transport::buffer::var_num::size_var_int;
    use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
    use crate::transport::packet::{PacketComponent, Size};

    pub struct Example {
        v_int: i32,
        uu: u8,
    }

    impl PacketComponent for Example {
        type ComponentType = Self;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
        where
            Self: Sized,
        {
            Box::pin(async move {
                let v_int = read.read_var_int().await?;
                let uu = read.read_u8().await?;
                Ok(Self { v_int, uu })
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                write.write_var_int(component_ref.v_int).await?;
                write.write_u8(component_ref.uu).await?;
                Ok(())
            })
        }

        fn size(input: &Self::ComponentType) -> Size {
            Size::Dynamic(size_var_int(input.v_int) + size_of::<u8>())
        }
    }

    #[tokio::test]
    async fn test_decode_packet() -> crate::Result<()> {
        let mut v = vec![25, 10];
        let mut cursor = Cursor::new(&mut v);
        let example = Example::decode(&mut cursor).await?;
        assert_eq!(example.v_int, 25);
        assert_eq!(example.uu, 10);
        Ok(())
    }

    #[tokio::test]
    async fn test_encode_packet() -> crate::Result<()> {
        let mut cursor = Cursor::new(vec![0; 2]);
        let example = Example { v_int: 25, uu: 10 };
        example.encode(&mut cursor).await?;
        assert_eq!(cursor.into_inner(), vec![25, 10]);
        Ok(())
    }

    #[tokio::test]
    async fn test_size_packet() -> crate::Result<()> {
        let example = Example { v_int: 25, uu: 10 };
        assert_eq!(Example::size(&example), Size::Dynamic(2));
        Ok(())
    }
}
