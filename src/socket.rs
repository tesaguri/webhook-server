use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub enum Listener {
    Tcp(tokio::net::TcpListener),
    #[cfg(unix)]
    Unix(tokio::net::UnixListener),
}

pub enum Stream {
    Tcp(tokio::net::TcpStream),
    #[cfg(unix)]
    Unix(tokio::net::UnixStream),
}

impl Listener {
    pub fn poll_accept(&self, cx: &mut Context<'_>) -> Poll<io::Result<Stream>> {
        match *self {
            Listener::Tcp(ref l) => l.poll_accept(cx).map_ok(|(sock, _)| Stream::Tcp(sock)),
            #[cfg(unix)]
            Listener::Unix(ref l) => l.poll_accept(cx).map_ok(|(sock, _)| Stream::Unix(sock)),
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match *self {
            Stream::Tcp(ref mut s) => Pin::new(s).poll_read(cx, buf),
            #[cfg(unix)]
            Stream::Unix(ref mut s) => Pin::new(s).poll_read(cx, buf),
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
            #[cfg(unix)]
            Stream::Unix(ref mut s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match *self {
            Stream::Tcp(ref mut s) => Pin::new(s).poll_flush(cx),
            #[cfg(unix)]
            Stream::Unix(ref mut s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        match *self {
            Stream::Tcp(ref mut s) => Pin::new(s).poll_shutdown(cx),
            #[cfg(unix)]
            Stream::Unix(ref mut s) => Pin::new(s).poll_shutdown(cx),
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        match *self {
            Stream::Tcp(ref mut s) => Pin::new(s).poll_write_vectored(cx, bufs),
            #[cfg(unix)]
            Stream::Unix(ref mut s) => Pin::new(s).poll_write_vectored(cx, bufs),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match *self {
            Stream::Tcp(ref s) => s.is_write_vectored(),
            #[cfg(unix)]
            Stream::Unix(ref s) => s.is_write_vectored(),
        }
    }
}
