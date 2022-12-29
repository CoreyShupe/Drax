use std::future::Future;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};

/// Defines a trait for defining a packet's component.
pub trait PacketComponent {
    /// Decodes the packet component from the given reader.
    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Self>> + 'a>>
    where
        Self: Sized;

    /// Encodes the packet component to the given writer.
    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        &'a self,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>>;

    /// Returns the known size of the packet component.
    fn size(&self) -> usize;
}

#[cfg(test)]
mod test {
    use std::future::Future;
    use std::io::Cursor;
    use std::pin::Pin;

    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

    use crate::transport::buffer::var_num::size_var_int;
    use crate::transport::buffer::{DraxReadExt, DraxWriteExt};
    use crate::transport::packet::PacketComponent;
    use crate::VarInt;

    pub struct Example {
        v_int: VarInt,
        uu: u8,
    }

    impl PacketComponent for Example {
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
            &'a self,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + 'a>> {
            Box::pin(async move {
                write.write_var_int(self.v_int).await?;
                write.write_u8(self.uu).await?;
                Ok(())
            })
        }

        fn size(&self) -> usize {
            size_var_int(self.v_int) + 1
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
        assert_eq!(example.size(), 2);
        Ok(())
    }
}
