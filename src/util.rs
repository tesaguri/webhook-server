use std::future::Future;
use std::ops::Deref;
use std::task::{Context, Poll};

use tower_service::Service;

pub struct DerefService<S>(pub S);

impl<S, T, R, E, F> Service<T> for DerefService<S>
where
    S: Deref,
    for<'a> &'a S::Target: Service<T, Response = R, Error = E, Future = F>,
    F: Future<Output = Result<R, E>>,
{
    type Response = R;
    type Error = E;
    type Future = F;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        (&*self.0).poll_ready(cx)
    }

    fn call(&mut self, req: T) -> Self::Future {
        (&*self.0).call(req)
    }
}
