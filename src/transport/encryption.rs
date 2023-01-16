use std::pin::Pin;
use std::task::{ready, Context, Poll};

pub use aes::cipher::AsyncStreamCipher;
pub use aes::cipher::NewCipher;
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, ReadBuf};

/// Encryption type alias for `cfb8::Encryptor<Aes128>`
pub type Cipher = cfb8::Cfb8<aes::Aes128>;

pin_project! {
    pub struct CipherAttachedReader<'a, R> {
        pub(crate) inner: &'a mut R,
        pub(crate) cipher: &'a mut Cipher,
    }
}

impl<'a, R: AsyncRead + Unpin> AsyncRead for CipherAttachedReader<'a, R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let me = self.project();
        ready!(Pin::new(me.inner).poll_read(cx, buf))?;
        me.cipher.decrypt(buf.filled_mut());
        Poll::Ready(Ok(()))
    }
}
