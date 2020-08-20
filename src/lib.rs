pub mod config;

mod service;
mod socket;
mod util;

pub use crate::config::Config;

use std::convert::TryInto;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_util::TryStreamExt;
use hyper::server::conn::Http;
use hyper::Body;
use listenfd::ListenFd;
use tokio::net::TcpListener;

use crate::service::Service;
use crate::socket::Listener;

pub struct Server {
    incoming: Listener,
    http: Http,
    service: Arc<Service<Body>>,
}

impl Server {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let incoming = if let Some(addr) = config.bind {
            Listener::Tcp(TcpListener::bind(addr).await?)
        } else {
            let mut fds = ListenFd::from_env();
            if let Some(i) = fds.take_tcp_listener(0).ok().flatten() {
                Listener::Tcp(i.try_into()?)
            } else if let Some(i) = fds.take_unix_listener(0).ok().flatten() {
                Listener::Unix(i.try_into()?)
            } else {
                anyhow::bail!("Either `bind` in config or `$LISTEN_FD` must be provided");
            }
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

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        while let Poll::Ready(option) = self.incoming.try_poll_next_unpin(cx)? {
            match option {
                None => return Poll::Ready(Ok(())),
                Some(io) => {
                    let service = util::DerefService(self.service.clone());
                    tokio::spawn(self.http.serve_connection(io, service));
                }
            }
        }
        Poll::Pending
    }
}
