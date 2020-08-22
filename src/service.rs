use std::collections::HashMap;
use std::fmt::Debug;
use std::io;
use std::marker::{PhantomData, Unpin};
use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Buf;
use futures_util::{future, stream, TryStreamExt};
use hmac::digest::generic_array::typenum::Unsigned;
use hmac::digest::FixedOutput;
use hmac::{Hmac, Mac, NewMac};
use http::header::HeaderName;
use http::StatusCode;
use http_body::Body;
use sha1::Sha1;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::config::{Config, DisplayHookCommand, Hook};

pub struct Service<B> {
    hooks: HashMap<Box<str>, Hook>,
    timeout: Duration,
    marker: PhantomData<fn() -> B>,
}

const X_HUB_SIGNATURE: &str = "x-hub-signature";

impl<B> Service<B>
where
    B: Body + Default + From<Vec<u8>> + Send + Unpin + 'static,
    B::Data: Send + Sync,
    B::Error: Debug + Send,
{
    pub fn new(config: Config) -> Self {
        Service {
            hooks: config.hook,
            timeout: config.timeout,
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

        let verifier = if let Some(secret) = hook.secret.as_ref() {
            let mac = Hmac::<Sha1>::new_varkey(secret.as_bytes()).unwrap();
            let signature =
                if let Some(v) = req.headers().get(HeaderName::from_static(X_HUB_SIGNATURE)) {
                    match parse_signature_header(v.as_bytes()) {
                        Ok(s) => s,
                        Err(SignatureParseError::Malformed) => {
                            return res
                                .status(StatusCode::BAD_REQUEST)
                                .body(Default::default())
                                .unwrap()
                        }
                        Err(SignatureParseError::UnknownAlgorithm) => {
                            return res
                                .status(StatusCode::NOT_ACCEPTABLE)
                                .body(Default::default())
                                .unwrap()
                        }
                    }
                } else {
                    return res
                        .status(StatusCode::UNAUTHORIZED)
                        .body(Default::default())
                        .unwrap();
                };
            Some((mac, signature))
        } else {
            None
        };

        let mut body = req.into_body();
        let body = stream::poll_fn(move |cx| Pin::new(&mut body).poll_data(cx));

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
            let body = body.try_fold(Vec::new(), |mut vec, chunk| {
                vec.extend(chunk.bytes());
                future::ok(vec)
            });
            let body = match body.await {
                Ok(body) => body,
                Err(e) => {
                    log::error!("Failed to read request body: {:?}", e);
                    return;
                }
            };
            if let Some((mut mac, signature)) = verifier {
                mac.update(&body);
                let code = mac.finalize().into_bytes();
                if *code != signature {
                    log::warn!("Signature mismatch");
                    return;
                }
            }
            if let Err(e) = stdin.write_all(&body).await {
                if e.kind() != io::ErrorKind::BrokenPipe {
                    log::error!("Failed to write to the pipe: {:?}", e);
                    return;
                }
            }
            if let Err(e) = stdin.shutdown().await {
                if e.kind() != io::ErrorKind::BrokenPipe {
                    log::error!("Failed to close the pipe: {:?}", e);
                    return;
                }
            }
            drop(stdin);

            let timeout = if timeout == Duration::from_secs(0) {
                future::Either::Right(future::pending())
            } else {
                future::Either::Left(tokio::time::delay_for(timeout))
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

const SIGNATURE_LEN: usize = <<Sha1 as FixedOutput>::OutputSize as Unsigned>::USIZE;

enum SignatureParseError {
    Malformed,
    UnknownAlgorithm,
}

fn parse_signature_header(header: &[u8]) -> Result<[u8; SIGNATURE_LEN], SignatureParseError> {
    let pos = header.iter().position(|&b| b == b'=');
    let (method, signature_hex) = if let Some(i) = pos {
        let (method, hex) = header.split_at(i);
        (method, &hex[1..])
    } else {
        return Err(SignatureParseError::Malformed);
    };

    match method {
        b"sha1" => {
            let mut buf = [0u8; SIGNATURE_LEN];
            hex::decode_to_slice(signature_hex, &mut buf)
                .map_err(|_| SignatureParseError::Malformed)?;
            Ok(buf)
        }
        _ => Err(SignatureParseError::UnknownAlgorithm),
    }
}
