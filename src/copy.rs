use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Debug)]
pub enum Direction {
    Request,  // 请求体
    Response, // 响应体
}


pub(crate) fn trace_leaf(_: &mut std::task::Context<'_>) -> std::task::Poll<()> {
    std::task::Poll::Ready(())
}

#[derive(Debug)]
pub(super) struct CopyBuffer {
    pub read_done: bool,
    pub need_flush: bool,
    pub pos: usize,
    pub cap: usize,
    pub amt: u64,
    pub buf: Box<[u8]>,
    pub direction: Direction, // 新增字段，表示当前数据流的方向
}

impl CopyBuffer {
    pub(super) fn new(buf_size: usize,direction: Direction) -> Self {
        Self {
            read_done: false,
            need_flush: false,
            pos: 0,
            cap: 0,
            amt: 0,
            buf: vec![0; buf_size].into_boxed_slice(),
            direction,
        }
    }

    fn poll_fill_buf<R>(
        &mut self,
        cx: &mut Context<'_>,
        reader: Pin<&mut R>,
    ) -> Poll<io::Result<()>>
    where
        R: AsyncRead + ?Sized,
    {
        let me = &mut *self;
        let mut buf = ReadBuf::new(&mut me.buf);
        buf.set_filled(me.cap);

        let res = reader.poll_read(cx, &mut buf);
        if let Poll::Ready(Ok(())) = res {
            let filled_len = buf.filled().len();
            me.read_done = me.cap == filled_len;
            me.cap = filled_len;
        }
        res
    }

    fn poll_write_buf<R, W>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
        mut writer: Pin<&mut W>,
    ) -> Poll<io::Result<usize>>
    where
        R: AsyncRead + ?Sized,
        W: AsyncWrite + ?Sized,
    {
        let me = &mut *self;
        match writer.as_mut().poll_write(cx, &me.buf[me.pos..me.cap]) {
            Poll::Pending => {
                // Top up the buffer towards full if we can read a bit more
                // data - this should improve the chances of a large write
                if !me.read_done && me.cap < me.buf.len() {
                    ready!(me.poll_fill_buf(cx, reader.as_mut()))?;
                }
                Poll::Pending
            }
            res => res,
        }
    }

    pub(super) fn poll_copy<R, W>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
        mut writer: Pin<&mut W>,
    ) -> Poll<io::Result<u64>>
    where
        R: AsyncRead + ?Sized,
        W: AsyncWrite + ?Sized,
    {
        ready!(trace_leaf(cx));
        #[cfg(any(
            feature = "fs",
            feature = "io-std",
            feature = "net",
            feature = "process",
            feature = "rt",
            feature = "signal",
            feature = "sync",
            feature = "time",
        ))]
        // Keep track of task budget
        let coop = ready!(crate::runtime::coop::poll_proceed(cx));
        loop {
            // If there is some space left in our buffer, then we try to read some
            // data to continue, thus maximizing the chances of a large write.
            if self.cap < self.buf.len() && !self.read_done {
                match self.poll_fill_buf(cx, reader.as_mut()) {
                    Poll::Ready(Ok(())) => {
                        #[cfg(any(
                            feature = "fs",
                            feature = "io-std",
                            feature = "net",
                            feature = "process",
                            feature = "rt",
                            feature = "signal",
                            feature = "sync",
                            feature = "time",
                        ))]
                        coop.made_progress();
                    }
                    Poll::Ready(Err(err)) => {
                        #[cfg(any(
                            feature = "fs",
                            feature = "io-std",
                            feature = "net",
                            feature = "process",
                            feature = "rt",
                            feature = "signal",
                            feature = "sync",
                            feature = "time",
                        ))]
                        coop.made_progress();
                        return Poll::Ready(Err(err));
                    }
                    Poll::Pending => {
                        // Ignore pending reads when our buffer is not empty, because
                        // we can try to write data immediately.
                        if self.pos == self.cap {
                            // Try flushing when the reader has no progress to avoid deadlock
                            // when the reader depends on buffered writer.
                            if self.need_flush {
                                ready!(writer.as_mut().poll_flush(cx))?;
                                #[cfg(any(
                                    feature = "fs",
                                    feature = "io-std",
                                    feature = "net",
                                    feature = "process",
                                    feature = "rt",
                                    feature = "signal",
                                    feature = "sync",
                                    feature = "time",
                                ))]
                                coop.made_progress();
                                self.need_flush = false;
                            }

                            return Poll::Pending;
                        }
                    }
                }
            }

            // If our buffer has some data, let's write it out!
            while self.pos < self.cap {
                let i = ready!(self.poll_write_buf(cx, reader.as_mut(), writer.as_mut()))?;
                #[cfg(any(
                    feature = "fs",
                    feature = "io-std",
                    feature = "net",
                    feature = "process",
                    feature = "rt",
                    feature = "signal",
                    feature = "sync",
                    feature = "time",
                ))]
                coop.made_progress();
                if i == 0 {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "write zero byte into writer",
                    )));
                } else {
                    self.pos += i;
                    self.amt += i as u64;
                    self.need_flush = true;
                }
            }

            // If pos larger than cap, this loop will never stop.
            // In particular, user's wrong poll_write implementation returning
            // incorrect written length may lead to thread blocking.
            debug_assert!(
                self.pos <= self.cap,
                "writer returned length larger than input slice"
            );

            // All data has been written, the buffer can be considered empty again
            self.pos = 0;
            self.cap = 0;

            // If we've written all the data and we've seen EOF, flush out the
            // data and finish the transfer.
            if self.read_done {
                ready!(writer.as_mut().poll_flush(cx))?;
                #[cfg(any(
                    feature = "fs",
                    feature = "io-std",
                    feature = "net",
                    feature = "process",
                    feature = "rt",
                    feature = "signal",
                    feature = "sync",
                    feature = "time",
                ))]
                coop.made_progress();
                return Poll::Ready(Ok(self.amt));
            }
        }
    }
}

