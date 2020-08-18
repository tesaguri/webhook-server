use std::collections::HashMap;
use std::fmt::Debug;
use std::io;
use std::marker::{PhantomData, Unpin};
use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Buf;
use futures_util::{future, stream, StreamExt};
use http::StatusCode;
use http_body::Body;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::config::{Config, DisplayHookCommand, Hook};

pub struct Service<B> {
    hooks: HashMap<Box<str>, Hook>,
    timeout: Option<Duration>,
    marker: PhantomData<fn() -> B>,
}

impl<B> Service<B>
where
    B: Body + Default + From<Vec<u8>> + Send + Unpin + 'static,
    B::Data: Send + Sync,
    B::Error: Debug + Send,
{
    pub fn new(config: Config) -> Self {
        Service {
            hooks: config.hook,
            timeout: config.timeout.map(|t| Duration::from_secs(t.get())),
            marker: PhantomData,
        }
    }

    fn call(&self, req: http::Request<B>) -> http::Response<B> {
        let res = http::Response::builder();

        let hook = if let Some(hook) = self.hooks.get(req.uri().path()) {
            hook
        } else {
            return res
                .status(StatusCode::NOT_FOUND)
                .body(B::default())
                .unwrap();
        };

        let mut body = req.into_body();
        let mut body = stream::poll_fn(move |cx| Pin::new(&mut body).poll_data(cx));

        log::info!("Executing a hook: {}", DisplayHookCommand(hook));

        let mut cmd = Command::new(&*hook.program);
        cmd.stdin(Stdio::piped());
        for arg in hook.args.as_deref().into_iter().flatten() {
            cmd.arg(&**arg);
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                if log::log_enabled!(log::Level::Error) {
                    let fmt = DisplayHookCommand(hook);
                    log::error!("Failed to execute command `{}`: {:?}", fmt, e);
                }
                return res
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(B::default())
                    .unwrap();
            }
        };

        let mut stdin = if let Some(stdin) = child.stdin.take() {
            stdin
        } else {
            log::error!("Failed to open stdin of child");
            return res
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(B::default())
                .unwrap();
        };

        let timeout = self.timeout;
        tokio::spawn(async move {
            while let Some(result) = body.next().await {
                let buf = match result {
                    Ok(buf) => buf,
                    Err(e) => {
                        log::error!("Failed to read request body: {:?}", e);
                        return;
                    }
                };
                if let Err(e) = stdin.write_all(&buf.bytes()).await {
                    if e.kind() != io::ErrorKind::BrokenPipe {
                        log::error!("Failed to write to the pipe: {:?}", e);
                        return;
                    }
                }
            }
            if let Err(e) = stdin.shutdown().await {
                if e.kind() != io::ErrorKind::BrokenPipe {
                    log::error!("Failed to close the pipe: {:?}", e);
                    return;
                }
            }
            drop(stdin);

            let timeout = if let Some(t) = timeout {
                future::Either::Left(tokio::time::delay_for(t))
            } else {
                future::Either::Right(future::pending())
            };
            use future::Either;
            match future::select(child, timeout).await {
                Either::Left((Ok(status), _)) => log::info!("Child exited. {}", status),
                Either::Left((Err(e), _)) => log::error!("Error waiting for child: {:?}", e),
                Either::Right((_, mut child)) => {
                    log::warn!("Timed out waiting for child");
                    let _ = child.kill();
                }
            }
        });

        res.body(B::default()).unwrap()
    }
}

impl<B> tower_service::Service<http::Request<B>> for &Service<B>
where
    B: Body + Default + From<Vec<u8>> + Send + Sync + Unpin + 'static,
    B::Data: Send + Sync,
    B::Error: Debug + Send,
{
    type Response = http::Response<B>;
    type Error = std::convert::Infallible;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        future::ok((*self).call(req))
    }
}
