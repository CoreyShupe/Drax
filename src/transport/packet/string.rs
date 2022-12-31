use std::future::Future;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::throw_explain;
use crate::transport::buffer::var_num::size_var_int;
use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
use crate::transport::packet::{LimitedPacketComponent, OwnedPacketComponent, Size};

const STRING_DEFAULT_CAP: i32 = 32767 * 4;

impl OwnedPacketComponent for String {
    fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
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

    fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
        &'a self,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        Box::pin(async move {
            write.write_var_int(self.len() as i32).await?;
            write.write_all(self.as_bytes()).await?;
            Ok(())
        })
    }

    fn size_owned(&self) -> Size {
        Size::Dynamic(self.len() + size_var_int(self.len() as i32))
    }
}

impl LimitedPacketComponent<i32> for String {
    fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
        limit: Option<i32>,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
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
