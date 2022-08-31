use aes::Aes128;
use cfb8::cipher::{AsyncStreamCipher, NewCipher};
use cfb8::Cfb8;
use pin_project_lite::pin_project;
use std::borrow::BorrowMut;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};

pub type EncryptionStream = Cfb8<Aes128>;

pin_project! {
    struct DecryptRead<R> {
        #[pin]
        read: R,
        #[pin]
        stream: EncryptionStream,
    }
}

macro_rules! ready {
    ($e:expr $(,)?) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        }
    };
}

impl<R: AsyncRead + Unpin + Sized> AsyncRead for DecryptRead<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut me = self.project();
        unsafe {
            let mut buf_read = ReadBuf::uninit(buf.unfilled_mut());

            ready!(Pin::new(&mut me.read).poll_read(cx, &mut buf_read)?);

            let filled_mut = buf_read.filled_mut();
            Pin::new(&mut me.stream).decrypt(filled_mut);

            let len = buf_read.filled().len();
            buf.assume_init(len);
            buf.advance(len);
        }
        Poll::Ready(Ok(()))
    }
}
