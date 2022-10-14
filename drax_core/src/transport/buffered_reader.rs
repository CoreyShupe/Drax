use crate::transport::pipeline::ShareChain;
use crate::transport::{Error, TransportProcessorContext};
use bytes::{Buf, BufMut, BytesMut};
use futures::ready;
use pin_project_lite::pin_project;
use std::future::Future;
use std::io::Cursor;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};

pub struct DraxTransportPipeline<T2> {
    pipeline: ShareChain<Vec<u8>, T2>,
    buffer: BytesMut,
}

impl<T2> DraxTransportPipeline<T2> {
    pub fn new(pipeline: ShareChain<Vec<u8>, T2>, buffer: BytesMut) -> Self {
        Self { pipeline, buffer }
    }

    pub fn read_transport_packet<'a, R>(
        &'a mut self,
        context: &'a mut TransportProcessorContext,
        reader: &'a mut R,
    ) -> ReadTransportPacket<T2, R> {
        ReadTransportPacket {
            pipeline: &mut self.pipeline,
            context,
            current_buffer: &mut self.buffer,
            reader,
            ready_size: None,
        }
    }

    pub fn update_chain(&mut self, chain: ShareChain<Vec<u8>, T2>) {
        self.pipeline = chain;
    }
}

pin_project! {
    pub struct ReadTransportPacket<'a, T, R> {
        pipeline: &'a mut super::pipeline::ShareChain<Vec<u8>, T>,
        context: &'a mut TransportProcessorContext,
        current_buffer: &'a mut BytesMut,
        reader: &'a mut R,
        #[pin]
        ready_size: Option<usize>,
    }
}

impl<'a, T, R> Future for ReadTransportPacket<'a, T, R>
where
    R: AsyncRead + Unpin,
{
    type Output = crate::transport::Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        // poll read buffer mostly from read_buf in tokio AsyncReadExt
        {
            use std::mem::MaybeUninit;

            if !me.current_buffer.has_remaining_mut() {
                return Poll::Ready(Error::cause("No packet found but buffer is full."));
            }

            let n = {
                let dst = me.current_buffer.chunk_mut();
                let dst = unsafe { &mut *(dst as *mut _ as *mut [MaybeUninit<u8>]) };
                let mut buf = ReadBuf::uninit(dst);
                let ptr = buf.filled().as_ptr();
                ready!(Pin::new(me.reader).poll_read(cx, &mut buf)?);

                // Ensure the pointer does not change from under us
                assert_eq!(ptr, buf.filled().as_ptr());
                buf.filled().len()
            };

            log::trace!("Read bytes: {}", n);

            if n == 0 {
                return Poll::Ready(Err(Error::EOF));
            }

            // Safety: This is guaranteed to be the number of initialized (and read)
            // bytes due to the invariants provided by `ReadBuf::filled`.
            unsafe {
                me.current_buffer.advance_mut(n);
            }
        }
        // check ready
        let size = match *me.ready_size {
            None => {
                let mut chunk_cursor = Cursor::new(me.current_buffer.chunk());
                match crate::extension::read_var_int_sync(
                    &mut TransportProcessorContext::default(),
                    &mut chunk_cursor,
                ) {
                    Ok(size) => {
                        let mut ready_size_inner = me.ready_size;
                        *ready_size_inner = Some(size as usize);
                        me.current_buffer.advance(chunk_cursor.position() as usize);
                        size as usize
                    }
                    Err(_) => {
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                }
            }
            Some(size) => size,
        };
        if size <= me.current_buffer.len() {
            let chunk_result = me
                .current_buffer
                .chunks(size)
                .next()
                .map(|inner| me.pipeline.process(me.context, inner.to_vec()))
                .unwrap_or_else(|| Error::cause("Failed to read buffer completely"));
            let capacity = me.current_buffer.capacity();
            let len = me.current_buffer.len();
            me.current_buffer.advance(size);
            me.current_buffer.reserve(capacity - len);
            Poll::Ready(chunk_result)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
