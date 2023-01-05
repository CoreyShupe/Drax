use std::fmt::format;
use std::future::Future;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;

use crate::throw_explain;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::transport::buffer::var_num::size_var_int;
use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
use crate::transport::packet::{PacketComponent, Size};

pub struct ByteDrain;

impl<C> PacketComponent<C> for ByteDrain {
    type ComponentType = Vec<u8>;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
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
        _: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_all(&component_ref).await?;
            Ok(())
        })
    }

    fn size(component_ref: &Self::ComponentType, _: &mut C) -> crate::prelude::Result<Size> {
        Ok(Size::Dynamic(component_ref.len()))
    }
}

pub struct SliceU8<const N: usize>;

impl<C, const N: usize> PacketComponent<C> for SliceU8<N> {
    type ComponentType = [u8; N];

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
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
        component_ref: &'a Self::ComponentType,
        _: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_all(component_ref).await?;
            Ok(())
        })
    }

    fn size(_: &Self::ComponentType, __: &mut C) -> crate::prelude::Result<Size> {
        Ok(Size::Constant(N))
    }
}

impl<C, T, const N: usize> PacketComponent<C> for [T; N]
where
    T: PacketComponent<C>,
{
    type ComponentType = [T::ComponentType; N];

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        context: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let mut arr: [MaybeUninit<T::ComponentType>; N] = MaybeUninit::uninit_array();
            for i in 0..N {
                arr[i] = MaybeUninit::new(T::decode(context, read).await?);
            }
            Ok(arr.map(|x| unsafe { x.assume_init() }))
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        context: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            for x in component_ref {
                T::encode(x, context, write).await?;
            }
            Ok(())
        })
    }

    fn size(component_ref: &Self::ComponentType, context: &mut C) -> crate::prelude::Result<Size> {
        let mut dynamic_counter = 0;
        for item in component_ref {
            match T::size(item, context)? {
                Size::Constant(x) => return Ok(Size::Constant(x * N)),
                Size::Dynamic(x) => dynamic_counter += x,
            }
        }
        Ok(Size::Dynamic(dynamic_counter))
    }
}

pub struct VecU8;

impl<C> PacketComponent<C> for VecU8 {
    type ComponentType = Vec<u8>;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
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
        component_ref: &'a Self::ComponentType,
        _: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_var_int(component_ref.len() as i32).await?;
            write.write_all(component_ref).await?;
            Ok(())
        })
    }

    fn size(component_ref: &Self::ComponentType, _: &mut C) -> crate::prelude::Result<Size> {
        Ok(Size::Dynamic(
            component_ref.len() + size_var_int(component_ref.len() as i32),
        ))
    }
}

impl<C, T> PacketComponent<C> for Vec<T>
where
    T: PacketComponent<C>,
{
    type ComponentType = Vec<T::ComponentType>;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        context: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
    where
        Self: Sized,
    {
        Box::pin(async move {
            let len = read.read_var_int().await?;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                vec.push(T::decode(context, read).await?);
            }
            Ok(vec)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        context: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_var_int(component_ref.len() as i32).await?;
            for item in component_ref {
                T::encode(item, context, write).await?;
            }
            Ok(())
        })
    }

    fn size(component_ref: &Self::ComponentType, context: &mut C) -> crate::prelude::Result<Size> {
        let var_int_size = size_var_int(component_ref.len() as i32);
        let mut dynamic_counter = var_int_size;
        for item in component_ref {
            match T::size(item, context)? {
                Size::Constant(x) => {
                    return Ok(Size::Dynamic((x * component_ref.len()) + var_int_size));
                }
                Size::Dynamic(x) => dynamic_counter += x,
            }
        }
        Ok(Size::Dynamic(dynamic_counter))
    }
}

pub struct LimitedVec<T, const N: usize>(PhantomData<T>);

impl<T, C, const N: usize> PacketComponent<C> for LimitedVec<T, N>
where
    T: PacketComponent<C>,
{
    type ComponentType = Vec<T::ComponentType>;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        context: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        Box::pin(async move {
            let vec_size = read.read_var_int().await? as usize;
            if vec_size > N {
                throw_explain!(format!(
                    "Tried to encode vec of length {} but was bound to length {}",
                    vec_size, N
                ));
            }

            let mut vec = Vec::with_capacity(vec_size);
            for _ in 0..vec_size {
                vec.push(T::decode(context, read).await?);
            }
            Ok(vec)
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        context: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        if component_ref.len() > N {
            return Box::pin(async move {
                throw_explain!(format!(
                    "Tried to encode vec of length {} but was bound to length {}.",
                    component_ref.len(),
                    N
                ))
            });
        }

        Vec::<T>::encode(component_ref, context, write)
    }

    fn size(input: &Self::ComponentType, context: &mut C) -> crate::prelude::Result<Size> {
        Vec::<T>::size(input, context)
    }
}
