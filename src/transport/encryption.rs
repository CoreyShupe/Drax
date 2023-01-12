use aes::Aes128;
use cfb8;
use cfb8::cipher::inout::InOutBuf;
use cfb8::cipher::{Block, BlockDecryptMut, BlockEncryptMut};
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Encryption type alias for `cfb8::Encryptor<Aes128>`
pub type Encryption = cfb8::Encryptor<Aes128>;
/// Decryption type alias for `cfb8::Decryptor<Aes128>`
pub type Decryption = cfb8::Decryptor<Aes128>;

pin_project! {
    /// A writer wrapper which encrypts all written data.
    pub struct EncryptedWriter<W> {
        #[pin]
        write: W,
        #[pin]
        stream: Option<Encryption>,
    }
}

impl<W> EncryptedWriter<W> {
    /// Create a new `EncryptedWriter` with the given writer and encryption stream.
    pub fn new(write: W, stream: Encryption) -> EncryptedWriter<W> {
        EncryptedWriter {
            write,
            stream: Some(stream),
        }
    }

    /// Create a new `EncryptedWriter` which does nothing except pass through.
    pub fn noop(write: W) -> EncryptedWriter<W> {
        EncryptedWriter {
            write,
            stream: None,
        }
    }

    pub fn into_inner(self) -> W {
        self.write
    }
}

impl<W: AsyncWrite + Unpin + Sized> AsyncWrite for EncryptedWriter<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut block_copy = buf.to_vec();
        let mut me = self.project();
        match me.stream.as_pin_mut() {
            None => Pin::new(&mut me.write).poll_write(cx, &block_copy),
            Some(stream) => {
                encrypt(stream, block_copy.as_mut_slice().into());
                Pin::new(&mut me.write).poll_write(cx, &block_copy)
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.project().write).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.project().write).poll_shutdown(cx)
    }
}

pin_project! {
    /// A reader wrapper which decrypts all read data.
    pub struct DecryptRead<R> {
        #[pin]
        read: R,
        #[pin]
        stream: Option<Decryption>,
    }
}

impl<R> DecryptRead<R> {
    /// Create a new `DecryptRead` with the given reader and decryption stream.
    pub fn new(read: R, stream: Decryption) -> DecryptRead<R> {
        DecryptRead {
            read,
            stream: Some(stream),
        }
    }

    /// Create a new `DecryptRead` which does nothing except pass through.
    pub fn noop(read: R) -> DecryptRead<R> {
        DecryptRead { read, stream: None }
    }

    pub fn into_inner(self) -> R {
        self.read
    }
}

impl<R: AsyncRead + Unpin + Sized> AsyncRead for DecryptRead<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut me = self.project();
        match me.stream.as_pin_mut() {
            Some(stream) => unsafe {
                let mut buf_read = ReadBuf::uninit(buf.unfilled_mut());

                ready!(Pin::new(&mut me.read).poll_read(cx, &mut buf_read)?);

                let filled_mut = buf_read.filled_mut();
                decrypt(stream, filled_mut.into());

                let len = buf_read.filled().len();
                buf.assume_init(len);
                buf.advance(len);
                Poll::Ready(Ok(()))
            },
            None => Pin::new(&mut me.read).poll_read(cx, buf),
        }
    }
}

fn encrypt(mut encryption: Pin<&mut Encryption>, data: InOutBuf<'_, '_, u8>) {
    let (blocks, mut tail) = data.into_chunks();
    encryption.encrypt_blocks_inout_mut(blocks);
    let n = tail.len();
    if n != 0 {
        let mut block = Block::<Encryption>::default();
        block[..n].copy_from_slice(tail.get_in());
        encryption.encrypt_block_mut(&mut block);
        tail.get_out().copy_from_slice(&block[..n]);
    }
}

fn decrypt(mut decryption: Pin<&mut Decryption>, data: InOutBuf<'_, '_, u8>) {
    let (blocks, mut tail) = data.into_chunks();
    decryption.decrypt_blocks_inout_mut(blocks);
    let n = tail.len();
    if n != 0 {
        let mut block = Block::<Decryption>::default();
        block[..n].copy_from_slice(tail.get_in());
        decryption.decrypt_block_mut(&mut block);
        tail.get_out().copy_from_slice(&block[..n]);
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::{DraxReadExt, DraxWriteExt};
    use crate::transport::encryption::{Decryption, Encryption};
    use cfb8::cipher::KeyIvInit;
    use std::io::Cursor;
    use std::pin::Pin;
    use tokio::io::AsyncReadExt;
    use tokio_test::assert_ok;

    #[tokio::test]
    async fn test_multi_block_cipher_persistence() {
        let key = [0x42; 16];
        let iv = [0x24; 16];

        let mut encryption = Encryption::new(&key.into(), &iv.into());
        let mut decryption = Decryption::new(&key.into(), &iv.into());

        let mut origin = vec![0u8, 5u8, 7u8, 10u8, 20u8];
        let dst = origin.clone();

        // run through gauntlet
        super::encrypt(Pin::new(&mut encryption), origin.as_mut_slice().into());
        assert_ne!(origin, dst);
        super::decrypt(Pin::new(&mut decryption), origin.as_mut_slice().into());
        assert_eq!(origin, dst);

        let mut origin = vec![2u8, 7u8, 12u8, 4u8, 1u8];
        let dst = origin.clone();

        // run through gauntlet again
        super::encrypt(Pin::new(&mut encryption), origin.as_mut_slice().into());
        assert_ne!(origin, dst);
        super::decrypt(Pin::new(&mut decryption), (&mut origin[0..2]).into());
        assert_eq!(origin[0..2], dst[0..2]);
        super::decrypt(Pin::new(&mut decryption), (&mut origin[2..5]).into());
        assert_eq!(origin[2..5], dst[2..5]);
    }

    #[tokio::test]
    async fn test_async_read_persistence() {
        let key = [0x42; 16];
        let iv = [0x24; 16];

        let encryption = Encryption::new(&key.into(), &iv.into());
        let decryption = Decryption::new(&key.into(), &iv.into());

        let mut input_cursor = Cursor::new(vec![1, 2, 3, 4, 5]);
        let mut output_cursor = Cursor::new(vec![0; 5]).encrypt_stream(encryption);
        assert_ok!(tokio::io::copy(&mut input_cursor, &mut output_cursor).await);

        let output_inner = output_cursor.into_inner().into_inner();
        assert_ne!(output_inner, vec![1, 2, 3, 4, 5]);

        let mut input_cursor = Cursor::new(output_inner).decrypt_stream(decryption);
        let mut output_buffer = [0u8; 5];
        assert_ok!(input_cursor.read_exact(&mut output_buffer).await);
        assert_eq!(output_buffer, [1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn test_encryption_2way_cipher() {
        let key = [0x42; 16];
        let iv = [0x24; 16];

        let mut encryption = Encryption::new(&key.into(), &iv.into());
        let mut decryption = Decryption::new(&key.into(), &iv.into());

        let mut origin = vec![0u8, 5u8, 7u8, 10u8, 20u8];
        let dst = origin.clone();

        // run through gauntlet
        super::encrypt(Pin::new(&mut encryption), origin.as_mut_slice().into());
        assert_ne!(origin, dst);
        super::decrypt(Pin::new(&mut decryption), origin.as_mut_slice().into());
        assert_eq!(origin, dst);

        let mut origin = vec![2u8, 7u8, 12u8, 4u8, 1u8];
        let dst = origin.clone();

        // run through gauntlet again
        super::encrypt(Pin::new(&mut encryption), origin.as_mut_slice().into());
        assert_ne!(origin, dst);
        super::decrypt(Pin::new(&mut decryption), origin.as_mut_slice().into());
        assert_eq!(origin, dst);
    }
}
