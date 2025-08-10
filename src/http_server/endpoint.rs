use std::future::Future;
use std::net::Incoming;
use std::sync::Arc;
use async_trait::async_trait;
use crate::errors::HttpResult;
use super::{Endpoint, Request, Response, Middleware, Next};

pub type DynEndpoint<Req, Resp> = dyn Endpoint<Req, Resp>;

pub(crate) struct MiddlewareEndpoint<E, Req: Request, Resp: Response> {
    endpoint: E,
    middleware: Vec<Arc<dyn Middleware<Req, Resp>>>,
}

impl<E: Clone, Req: Request, Resp: Response> Clone for MiddlewareEndpoint<E, Req, Resp> {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            middleware: self.middleware.clone(),
        }
    }
}

impl<E, Req: Request, Resp: Response> std::fmt::Debug for MiddlewareEndpoint<E, Req, Resp> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            fmt,
            "MiddlewareEndpoint (length: {})",
            self.middleware.len(),
        )
    }
}

impl<E, Req: Request, Resp: Response> MiddlewareEndpoint<E, Req, Resp>
where
    E: Endpoint<Req, Resp>,
{
    pub(crate) fn wrap_with_middleware(
        ep: E,
        middleware: &[Arc<dyn Middleware<Req, Resp>>],
    ) -> Box<dyn Endpoint<Req, Resp> + Send + Sync + 'static> {
        if middleware.is_empty() {
            Box::new(ep)
        } else {
            Box::new(Self {
                endpoint: ep,
                middleware: middleware.to_vec(),
            })
        }
    }
}

#[async_trait]
impl<E, Req: Request, Resp: Response> Endpoint<Req, Resp> for MiddlewareEndpoint<E, Req, Resp>
where
    E: Endpoint<Req, Resp>,
{
    async fn call(&self, req: Req) -> HttpResult<Resp> {
        let next = Next {
            endpoint: &self.endpoint,
            next_middleware: &self.middleware,
        };
        Ok(next.run(req).await)
    }
}

#[async_trait]
impl<Req: Request, Resp: Response> Endpoint<Req, Resp> for Box<dyn Endpoint<Req, Resp>> {
    async fn call(&self, request: Req) -> HttpResult<Resp> {
        self.as_ref().call(request).await
    }
}
