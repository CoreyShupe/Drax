use aes::Aes128;
use cfb8::cipher::AsyncStreamCipher;
use cfb8::Cfb8;
use futures::ready;
use pin_project_lite::pin_project;
use std::io::Error;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub type EncryptionStream = Cfb8<Aes128>;

pin_project! {
    pub struct EncryptedWriter<W> {
        #[pin]
        write: W,
        #[pin]
        stream: Option<EncryptionStream>,
    }
}

impl<W> EncryptedWriter<W> {
    pub fn new(write: W, stream: EncryptionStream) -> EncryptedWriter<W> {
        EncryptedWriter {
            write,
            stream: Some(stream),
        }
    }
}

impl<W: AsyncWrite + Unpin + Sized> AsyncWrite for EncryptedWriter<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        let mut block_copy = buf.to_vec();
        let mut me = self.project();
        match (*me.stream).as_mut() {
            None => Pin::new(&mut me.write).poll_write(cx, &block_copy),
            Some(stream) => {
                stream.encrypt(&mut block_copy);
                Pin::new(&mut me.write).poll_write(cx, &block_copy)
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.project().write).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.project().write).poll_shutdown(cx)
    }
}

pin_project! {
    pub struct DecryptRead<R> {
        #[pin]
        read: R,
        #[pin]
        stream: Option<EncryptionStream>,
    }
}

impl<R> DecryptRead<R> {
    pub fn new(read: R, stream: EncryptionStream) -> DecryptRead<R> {
        DecryptRead {
            read,
            stream: Some(stream),
        }
    }
}

impl<R: AsyncRead + Unpin + Sized> AsyncRead for DecryptRead<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut me = self.project();
        match (*me.stream).as_mut() {
            Some(stream) => unsafe {
                let mut buf_read = ReadBuf::uninit(buf.unfilled_mut());

                ready!(Pin::new(&mut me.read).poll_read(cx, &mut buf_read)?);

                let filled_mut = buf_read.filled_mut();
                Pin::new(stream).decrypt(filled_mut);

                let len = buf_read.filled().len();
                buf.assume_init(len);
                buf.advance(len);
                Poll::Ready(Ok(()))
            },
            None => Pin::new(&mut me.read).poll_read(cx, buf),
        }
    }
}
