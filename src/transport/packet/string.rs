use std::future::Future;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::throw_explain;
use crate::transport::buffer::var_num::size_var_int;
use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
use crate::transport::packet::{PacketComponent, Size};

const STRING_DEFAULT_CAP: i32 = 32767 * 4;

impl<C> PacketComponent<C> for String {
    type ComponentType = Self;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
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
        _: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_var_int(component_ref.len() as i32).await?;
            write.write_all(component_ref.as_bytes()).await?;
            Ok(())
        })
    }

    fn size(component_ref: &Self, _: &mut C) -> crate::prelude::Result<Size> {
        Ok(Size::Dynamic(
            component_ref.len() + size_var_int(component_ref.len() as i32),
        ))
    }
}
