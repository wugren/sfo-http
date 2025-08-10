use route_recognizer::{Match, Params, Router as MethodRouter};
use std::collections::HashMap;
use http::{Method, StatusCode};
use crate::errors::HttpResult;
use super::{DynEndpoint, Request, Response};

pub(crate) struct Router<Req: Request, Resp: Response> {
    method_map: HashMap<Method, MethodRouter<Box<DynEndpoint<Req, Resp>>>>,
    all_method_router: MethodRouter<Box<DynEndpoint<Req, Resp>>>,
}

/// The result of routing a URL
#[allow(missing_debug_implementations)]
pub(crate) struct Selection<'a, Req: Request, Resp: Response> {
    pub(crate) endpoint: &'a DynEndpoint<Req, Resp>,
    pub(crate) params: Params,
}

impl<Req: Request, Resp: Response> Router<Req, Resp> {
    pub(crate) fn new() -> Self {
        Router {
            method_map: HashMap::default(),
            all_method_router: MethodRouter::new(),
        }
    }

    pub(crate) fn add(
        &mut self,
        path: &str,
        method: Method,
        ep: Box<DynEndpoint<Req, Resp>>,
    ) {
        self.method_map
            .entry(method)
            .or_insert_with(MethodRouter::new)
            .add(path, ep)
    }

    pub(crate) fn add_all(&mut self, path: &str, ep: Box<DynEndpoint<Req, Resp>>) {
        self.all_method_router.add(path, ep)
    }

    pub(crate) fn route(&self, path: &str, method: Method) -> Selection<Req, Resp> {
        if let Some(matcher) = self
            .method_map
            .get(&method)
            .and_then(|r| r.recognize(path).ok())
        {
            let handler = matcher.handler();
            let params = matcher.params().clone();
            Selection {
                endpoint: &**handler,
                params,
            }
        } else if let Ok(matcher) = self.all_method_router.recognize(path) {
            let handler = matcher.handler();
            let params = matcher.params().clone();
            Selection {
                endpoint: &**handler,
                params,
            }
        } else if method == Method::HEAD {
            // If it is a HTTP HEAD request then check if there is a callback in the endpoints map
            // if not then fallback to the behavior of HTTP GET else proceed as usual

            self.route(path, Method::GET)
        } else if self
            .method_map
            .iter()
            .filter(|(k, _)| **k != method)
            .any(|(_, r)| r.recognize(path).is_ok())
        {
            // If this `path` can be handled by a callback registered with a different HTTP method
            // should return 405 Method Not Allowed
            Selection {
                endpoint: &method_not_allowed,
                params: Params::new(),
            }
        } else {
            Selection {
                endpoint: &not_found_endpoint,
                params: Params::new(),
            }
        }
    }
}

async fn not_found_endpoint<Req: Request, Resp: Response>(
    _req: Req,
) -> HttpResult<Resp> {
    Ok(Resp::new(StatusCode::NOT_FOUND))
}

async fn method_not_allowed<Req: Request, Resp: Response>(
    _req: Req,
) -> HttpResult<Resp> {
    Ok(Resp::new(StatusCode::METHOD_NOT_ALLOWED))
}
