use std::future::Future;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::transport::buffer::var_num::size_var_int;
use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
use crate::transport::packet::{
    LimitedPacketComponent, OwnedPacketComponent, PacketComponent, Size,
};

pub struct ByteDrain;

impl PacketComponent for ByteDrain {
    type ComponentType = Vec<u8>;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let mut bytes = vec![];
            read.read_to_end(&mut bytes).await?;
            Ok(bytes)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_all(&component_ref).await?;
            Ok(())
        })
    }

    fn size(input: &Self::ComponentType) -> Size {
        Size::Dynamic(input.len())
    }
}

impl<const N: usize> OwnedPacketComponent for [u8; N] {
    fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let mut buf = [0; N];
            read.read_exact(&mut buf).await?;
            Ok(buf)
        })
    }

    fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
        &'a self,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_all(self).await?;
            Ok(())
        })
    }

    fn size_owned(&self) -> Size {
        Size::Constant(N)
    }
}

impl<T, const N: usize> OwnedPacketComponent for [T; N]
where
    T: OwnedPacketComponent,
{
    fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let mut arr: [MaybeUninit<T>; N] = MaybeUninit::uninit_array();
            for i in 0..N {
                arr[i] = MaybeUninit::new(T::decode_owned(read).await?);
            }
            Ok(arr.map(|x| unsafe { x.assume_init() }))
        })
    }

    fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
        &'a self,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            for x in self {
                x.encode_owned(write).await?;
            }
            Ok(())
        })
    }

    fn size_owned(&self) -> Size {
        let mut dynamic_counter = 0;
        for item in self {
            match item.size_owned() {
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
    T: OwnedPacketComponent,
    L: Copy,
{
    fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
        limit: Option<L>,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
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

pub struct DelegateSlice<T, const N: usize> {
    _phantom_t: PhantomData<T>,
}

impl<T, const N: usize> PacketComponent for DelegateSlice<T, N>
where
    T: PacketComponent,
{
    type ComponentType = [T::ComponentType; N];

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move {
            let mut arr: [MaybeUninit<T::ComponentType>; N] = MaybeUninit::uninit_array();
            for i in 0..N {
                arr[i] = MaybeUninit::new(T::decode(read).await?);
            }
            Ok(arr.map(|x| unsafe { x.assume_init() }))
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            for x in component_ref {
                T::encode(x, write).await?;
            }
            Ok(())
        })
    }

    fn size(input: &Self::ComponentType) -> Size {
        let mut dynamic_counter = 0;
        for item in input {
            match T::size(item) {
                Size::Constant(x) => return Size::Constant(x * N),
                Size::Dynamic(x) => dynamic_counter += x,
            }
        }
        Size::Dynamic(dynamic_counter)
    }
}

impl<T, Limit, const N: usize> LimitedPacketComponent<Limit> for DelegateSlice<T, N>
where
    T: LimitedPacketComponent<Limit>,
    Limit: Copy,
{
    fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
        limit: Option<Limit>,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
    where
        Limit: 'a,
    {
        Box::pin(async move {
            let mut arr: [MaybeUninit<T::ComponentType>; N] = MaybeUninit::uninit_array();
            for i in 0..N {
                arr[i] = MaybeUninit::new(T::decode_with_limit(read, limit).await?);
            }
            Ok(arr.map(|x| unsafe { x.assume_init() }))
        })
    }
}

impl OwnedPacketComponent for Vec<u8> {
    fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
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

    fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
        &'a self,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_var_int(self.len() as i32).await?;
            write.write_all(self).await?;
            Ok(())
        })
    }

    fn size_owned(&self) -> Size {
        Size::Dynamic(self.len() + size_var_int(self.len() as i32))
    }
}

impl<T> OwnedPacketComponent for Vec<T>
where
    T: OwnedPacketComponent,
{
    fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let len = read.read_var_int().await?;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                vec.push(T::decode_owned(read).await?);
            }
            Ok(vec)
        })
    }

    fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
        &'a self,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_var_int(self.len() as i32).await?;
            for item in self {
                item.encode_owned(write).await?;
            }
            Ok(())
        })
    }

    fn size_owned(&self) -> Size {
        let var_int_size = size_var_int(self.len() as i32);
        let mut dynamic_counter = var_int_size;
        for item in self {
            match item.size_owned() {
                Size::Constant(x) => return Size::Dynamic((x * self.len()) + var_int_size),
                Size::Dynamic(x) => dynamic_counter += x,
            }
        }
        Size::Dynamic(dynamic_counter)
    }
}

impl<T, N> LimitedPacketComponent<N> for Vec<T>
where
    T: LimitedPacketComponent<N> + OwnedPacketComponent + PacketComponent<ComponentType = T>,
    N: Copy,
{
    fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
        limit: Option<N>,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
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

pub struct VecDelegate<T> {
    _phantom_t: PhantomData<T>,
}

impl<T> PacketComponent for VecDelegate<T>
where
    T: PacketComponent,
{
    type ComponentType = Vec<T::ComponentType>;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
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
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_var_int(component_ref.len() as i32).await?;
            for item in component_ref {
                T::encode(item, write).await?;
            }
            Ok(())
        })
    }

    fn size(input: &Self::ComponentType) -> Size {
        let var_int_size = size_var_int(input.len() as i32);
        let mut dynamic_counter = var_int_size;
        for item in input {
            match T::size(item) {
                Size::Constant(x) => return Size::Dynamic((x * input.len()) + var_int_size),
                Size::Dynamic(x) => dynamic_counter += x,
            }
        }
        Size::Dynamic(dynamic_counter)
    }
}
