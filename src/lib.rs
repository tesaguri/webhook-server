pub mod config;

mod service;
mod socket;
mod util;

pub use crate::config::Config;

use std::convert::TryInto;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use hyper::server::conn::Http;
use listenfd::ListenFd;
use tokio::net::TcpListener;

use crate::service::Service;
use crate::socket::Listener;

pub struct Server {
    incoming: Listener,
    http: Http,
    service: Arc<Service>,
}

impl Server {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let incoming = if let Some(addr) = config.bind {
            Listener::Tcp(TcpListener::bind(addr).await?)
        } else if let Some(l) = listen_fd()? {
            l
        } else {
            anyhow::bail!("Either `bind` in config or `$LISTEN_FD` must be provided");
        };

        Ok(Server {
            incoming,
            http: Http::new(),
            service: Arc::new(Service::new(config)),
        })
    }
}

impl Future for Server {
    type Output = anyhow::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        while let Poll::Ready(io) = self.incoming.poll_accept(cx)? {
            let service = util::DerefService(self.service.clone());
            tokio::spawn(self.http.serve_connection(io, service));
        }
        Poll::Pending
    }
}

fn listen_fd() -> io::Result<Option<Listener>> {
    let mut fds = ListenFd::from_env();
    if let Some(l) = fds.take_tcp_listener(0).ok().flatten() {
        return Ok(Some(Listener::Tcp(l.try_into()?)));
    }
    #[cfg(unix)]
    if let Some(l) = fds.take_unix_listener(0).ok().flatten() {
        return Ok(Some(Listener::Unix(l.try_into()?)));
    }
    Ok(None)
}
