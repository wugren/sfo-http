//! Middleware types.

use std::sync::Arc;

use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use http::StatusCode;
use route_recognizer::nfa::State;
use crate::errors::HttpResult;
use crate::http_server::endpoint::DynEndpoint;
use super::{Endpoint, Request, Response,};

/// Middleware that wraps around the remaining middleware chain.
#[async_trait]
pub trait Middleware<Req: Request, Resp: Response>: Send + Sync + 'static {
    /// Asynchronously handle the request, and return a response.
    async fn handle(&self, request: Req, next: Next<'_, Req, Resp>) -> HttpResult<Resp>;

    /// Set the middleware's name. By default it uses the type signature.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

#[async_trait]
impl<Req, Resp, F> Middleware<Req, Resp> for F
where
    Req: Request,
    Resp: Response,
    F: Send
        + Sync
        + 'static
        + for<'a> Fn(
            Req,
            Next<'a, Req, Resp>,
        ) -> Pin<Box<dyn Future<Output = HttpResult<Resp>> + 'a + Send>>,
{
    async fn handle(&self, req: Req, next: Next<'_, Req, Resp>) -> HttpResult<Resp> {
        (self)(req, next).await
    }
}

/// The remainder of a middleware chain, including the endpoint.
#[allow(missing_debug_implementations)]
pub struct Next<'a, Req: Request, Resp: Response> {
    pub(crate) endpoint: &'a DynEndpoint<Req, Resp>,
    pub(crate) next_middleware: &'a [Arc<dyn Middleware< Req, Resp>>],
}

impl<Req: Request, Resp: Response> Next<'_, Req, Resp> {
    /// Asynchronously execute the remaining middleware chain.
    pub async fn run(mut self, req: Req) -> Resp {
        if let Some((current, next)) = self.next_middleware.split_first() {
            self.next_middleware = next;
            match current.handle(req, self).await {
                Ok(request) => request,
                Err(err) => {
                    log::error!("middleware handle err: {}", err);
                    Resp::new(StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        } else {
            match self.endpoint.call(req).await {
                Ok(request) => request,
                Err(err) => {
                    log::error!("endpoint call err: {}", err);
                    Resp::new(StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        }
    }
}
