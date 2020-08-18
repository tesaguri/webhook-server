use std::io;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::BufMut;
use futures_util::TryStreamExt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};

pub enum Listener {
    Tcp(TcpListener),
    Unix(UnixListener),
}

pub enum Stream {
    Tcp(TcpStream),
    Unix(UnixStream),
}

impl futures_core::Stream for Listener {
    type Item = Result<Stream, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match *self {
            Listener::Tcp(ref mut l) => l
                .try_poll_next_unpin(cx)
                .map(|result| result.map(|opt| opt.map(Stream::Tcp))),
            Listener::Unix(ref mut l) => l
                .try_poll_next_unpin(cx)
                .map(|result| result.map(|opt| opt.map(Stream::Unix))),
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match *self {
            Stream::Tcp(ref mut s) => Pin::new(s).poll_read(cx, buf),
            Stream::Unix(ref mut s) => Pin::new(s).poll_read(cx, buf),
        }
    }

    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [MaybeUninit<u8>]) -> bool {
        match *self {
            Stream::Tcp(ref s) => s.prepare_uninitialized_buffer(buf),
            Stream::Unix(ref s) => s.prepare_uninitialized_buffer(buf),
        }
    }

    fn poll_read_buf<B: BufMut>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>> {
        match *self {
            Stream::Tcp(ref mut s) => Pin::new(s).poll_read_buf(cx, buf),
            Stream::Unix(ref mut s) => Pin::new(s).poll_read_buf(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match *self {
            Stream::Tcp(ref mut s) => Pin::new(s).poll_write(cx, buf),
            Stream::Unix(ref mut s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match *self {
            Stream::Tcp(ref mut s) => Pin::new(s).poll_flush(cx),
            Stream::Unix(ref mut s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        match *self {
            Stream::Tcp(ref mut s) => Pin::new(s).poll_shutdown(cx),
            Stream::Unix(ref mut s) => Pin::new(s).poll_shutdown(cx),
        }
    }

    fn poll_write_buf<B: bytes::Buf>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>> {
        match *self {
            Stream::Tcp(ref mut s) => Pin::new(s).poll_write_buf(cx, buf),
            Stream::Unix(ref mut s) => Pin::new(s).poll_write_buf(cx, buf),
        }
    }
}
