use std::pin::Pin;
use std::task::{ready, Context, Poll};

use aes::cipher::{AsyncStreamCipher, NewCipher};
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Encryption type alias for `cfb8::Encryptor<Aes128>`
type Cipher = cfb8::Cfb8<aes::Aes128>;

pin_project! {
    /// A writer wrapper which encrypts all written data.
    pub struct EncryptedWriter<W> {
        #[pin]
        write: W,
        #[pin]
        cipher: Option<Cipher>,
    }
}

impl<W> EncryptedWriter<W> {
    /// Create a new `EncryptedWriter` with the given writer and encryption stream.
    pub fn new(write: W, cipher_key: &[u8]) -> EncryptedWriter<W> {
        EncryptedWriter {
            write,
            cipher: Some(NewCipher::new_from_slices(cipher_key, cipher_key).unwrap()),
        }
    }

    /// Create a new `EncryptedWriter` which does nothing except pass through.
    pub fn noop(write: W) -> EncryptedWriter<W> {
        EncryptedWriter {
            write,
            cipher: None,
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
        match me.cipher.as_pin_mut() {
            None => Pin::new(&mut me.write).poll_write(cx, &block_copy),
            Some(mut cipher) => {
                cipher.encrypt(&mut block_copy);
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
        cipher: Option<Cipher>,
    }
}

impl<R> DecryptRead<R> {
    /// Create a new `DecryptRead` with the given reader and decryption stream.
    pub fn new(read: R, cipher_key: &[u8]) -> DecryptRead<R> {
        DecryptRead {
            read,
            cipher: Some(NewCipher::new_from_slices(cipher_key, cipher_key).unwrap()),
        }
    }

    /// Create a new `DecryptRead` which does nothing except pass through.
    pub fn noop(read: R) -> DecryptRead<R> {
        DecryptRead { read, cipher: None }
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
        match me.cipher.as_pin_mut() {
            Some(mut cipher) => unsafe {
                let mut buf_read = ReadBuf::uninit(buf.unfilled_mut());

                ready!(Pin::new(&mut me.read).poll_read(cx, &mut buf_read)?);

                let filled_mut = buf_read.filled_mut();
                cipher.decrypt(filled_mut);

                let len = buf_read.filled().len();
                buf.assume_init(len);
                buf.advance(len);
                Poll::Ready(Ok(()))
            },
            None => Pin::new(&mut me.read).poll_read(cx, buf),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use tokio::io::AsyncReadExt;
    use tokio_test::assert_ok;

    use crate::prelude::{DraxReadExt, DraxWriteExt};

    #[tokio::test]
    async fn test_async_read_persistence() {
        let key = [0x42; 16];

        let mut input_cursor = Cursor::new(vec![1, 2, 3, 4, 5]);
        let mut output_cursor = Cursor::new(vec![0; 5]).encrypt_stream(&key);
        assert_ok!(tokio::io::copy(&mut input_cursor, &mut output_cursor).await);

        let output_inner = output_cursor.into_inner().into_inner();
        assert_ne!(output_inner, vec![1, 2, 3, 4, 5]);

        let mut input_cursor = Cursor::new(output_inner).decrypt_stream(&key);
        let mut output_buffer = [0u8; 5];
        assert_ok!(input_cursor.read_exact(&mut output_buffer).await);
        assert_eq!(output_buffer, [1, 2, 3, 4, 5]);
    }
}
