pub mod config;

mod service;
mod util;

pub use crate::config::Config;

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_util::TryStreamExt;
use hyper::server::conn::Http;
use hyper::Body;
use tokio::net::TcpListener;

use crate::service::Service;

pin_project_lite::pin_project! {
    pub struct Server {
        incoming: TcpListener,
        http: Http,
        service: Arc<Service<Body>>,
    }
}

impl Server {
    pub async fn new(config: Config) -> io::Result<Self> {
        let incoming = TcpListener::bind((config.address, config.port)).await?;
        let service = Arc::new(Service::new(config));

        Ok(Server {
            http: Http::new(),
            incoming,
            service,
        })
    }
}

impl Future for Server {
    type Output = anyhow::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        while let Poll::Ready(option) = this.incoming.try_poll_next_unpin(cx)? {
            match option {
                None => return Poll::Ready(Ok(())),
                Some(sock) => {
                    let service = util::DerefService(this.service.clone());
                    tokio::spawn(this.http.serve_connection(sock, service));
                }
            }
        }
        Poll::Pending
    }
}
