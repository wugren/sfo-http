use std::fmt::Debug;
use std::io;
use std::path::Path;
use std::sync::Arc;
use http::Method;
use crate::errors::HttpResult;
use super::{Endpoint, Middleware, MiddlewareEndpoint, Request, Response, Router, ServeDir, ServeFile};

#[allow(missing_debug_implementations)]
pub struct Route<'a, Req: Request, Resp: Response> {
    router: &'a mut Router<Req, Resp>,
    path: String,
    middleware: Vec<Arc<dyn Middleware<Req, Resp>>>,
    prefix: bool,
}

impl<'a, Req: Request, Resp: Response> Route<'a, Req, Resp> {
    pub(crate) fn new(router: &'a mut Router<Req, Resp>, path: String) -> Route<'a, Req, Resp> {
        Route {
            router,
            path,
            middleware: Vec::new(),
            prefix: false,
        }
    }

    /// Extend the route with the given `path`.
    pub fn at<'b>(&'b mut self, path: &str) -> Route<'b, Req, Resp> {
        let mut p = self.path.clone();

        if !p.ends_with('/') && !path.starts_with('/') {
            p.push('/');
        }

        if path != "/" {
            p.push_str(path);
        }

        Route {
            router: &mut self.router,
            path: p,
            middleware: self.middleware.clone(),
            prefix: false,
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn with<M>(&mut self, middleware: M) -> &mut Self
    where
        M: Middleware<Req, Resp>,
    {
        log::trace!(
            "Adding middleware {} to route {:?}",
            middleware.name(),
            self.path
        );
        self.middleware.push(Arc::new(middleware));
        self
    }

    pub fn reset_middleware(&mut self) -> &mut Self {
        self.middleware.clear();
        self
    }

    pub fn serve_dir(&mut self, dir: impl AsRef<Path>) -> io::Result<()> {
        // Verify path exists, return error if it doesn't.
        let dir = dir.as_ref().to_owned().canonicalize()?;
        let prefix = self.path().to_string();
        self.at("*").get(ServeDir::new(prefix, dir));
        Ok(())
    }

    pub fn serve_file(&mut self, file: impl AsRef<Path>) -> io::Result<()> {
        self.get(ServeFile::init(file)?);
        Ok(())
    }

    pub fn method(&mut self, method: Method, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        if self.prefix {
            let ep = StripPrefixEndpoint::new(ep);

            self.router.add(
                &self.path,
                method.clone(),
                MiddlewareEndpoint::wrap_with_middleware(ep.clone(), &self.middleware),
            );
            let wildcard = self.at("*--tide-path-rest");
            wildcard.router.add(
                &wildcard.path,
                method,
                MiddlewareEndpoint::wrap_with_middleware(ep, &wildcard.middleware),
            );
        } else {
            self.router.add(
                &self.path,
                method,
                MiddlewareEndpoint::wrap_with_middleware(ep, &self.middleware),
            );
        }
        self
    }

    pub fn all(&mut self, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        if self.prefix {
            let ep = StripPrefixEndpoint::new(ep);

            self.router.add_all(
                &self.path,
                MiddlewareEndpoint::wrap_with_middleware(ep.clone(), &self.middleware),
            );
            let wildcard = self.at("*--tide-path-rest");
            wildcard.router.add_all(
                &wildcard.path,
                MiddlewareEndpoint::wrap_with_middleware(ep, &wildcard.middleware),
            );
        } else {
            self.router.add_all(
                &self.path,
                MiddlewareEndpoint::wrap_with_middleware(ep, &self.middleware),
            );
        }
        self
    }

    /// Add an endpoint for `GET` requests
    pub fn get(&mut self, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        self.method(Method::GET, ep);
        self
    }

    /// Add an endpoint for `HEAD` requests
    pub fn head(&mut self, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        self.method(Method::HEAD, ep);
        self
    }

    /// Add an endpoint for `PUT` requests
    pub fn put(&mut self, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        self.method(Method::PUT, ep);
        self
    }

    /// Add an endpoint for `POST` requests
    pub fn post(&mut self, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        self.method(Method::POST, ep);
        self
    }

    /// Add an endpoint for `DELETE` requests
    pub fn delete(&mut self, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        self.method(Method::DELETE, ep);
        self
    }

    /// Add an endpoint for `OPTIONS` requests
    pub fn options(&mut self, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        self.method(Method::OPTIONS, ep);
        self
    }

    /// Add an endpoint for `CONNECT` requests
    pub fn connect(&mut self, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        self.method(Method::CONNECT, ep);
        self
    }

    /// Add an endpoint for `PATCH` requests
    pub fn patch(&mut self, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        self.method(Method::PATCH, ep);
        self
    }

    /// Add an endpoint for `TRACE` requests
    pub fn trace(&mut self, ep: impl Endpoint<Req, Resp>) -> &mut Self {
        self.method(Method::TRACE, ep);
        self
    }
}

#[derive(Debug)]
struct StripPrefixEndpoint<E>(std::sync::Arc<E>);

impl<E> StripPrefixEndpoint<E> {
    fn new(ep: E) -> Self {
        Self(std::sync::Arc::new(ep))
    }
}

impl<E> Clone for StripPrefixEndpoint<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[async_trait::async_trait]
impl<Req: Request, Resp: Response, E> Endpoint<Req, Resp> for StripPrefixEndpoint<E>
where
    E: Endpoint<Req, Resp>,
{
    async fn call(&self, req: Req) -> HttpResult<Resp> {
        self.0
            .call(req)
            .await
    }
}
