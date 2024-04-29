use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use actix_web::{FromRequest, Handler, HttpMessage, HttpRequest, HttpResponse, Responder, web};
use actix_web::body::BoxBody;
use actix_web::dev::{Payload, Service, ServiceRequest, ServiceResponse, Url};
use actix_web::http::{Method, StatusCode, Version};
use actix_web::http::header::{HeaderName, HeaderValue};
use futures_util::future::LocalBoxFuture;
use futures_util::stream::IntoAsyncRead;
use futures_util::{AsyncReadExt, StreamExt, TryStreamExt};
use serde::de::DeserializeOwned;
use crate::actix_server::body::{BodySize, MessageBody};
use crate::errors::{ErrorCode, http_err, HttpError, HttpResult, into_http_err};

pub struct Request<State> {
    state: State,
    request: HttpRequest,
    payload: Option<Payload>,
}

impl<State> Request<State> {
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn method(&self) -> Method {
        self.request.method().clone()
    }

    pub fn url(&self) -> &Url {
        self.request.match_info().get_ref()
    }

    pub fn version(&self) -> Option<Version> {
        Some(self.request.version())
    }

    pub fn peer_addr(&self) -> Option<String> {
        self.request.peer_addr().map(|addr| addr.to_string())
    }

    pub fn local_addr(&self) -> Option<String> {
        Some(self.request.app_config().local_addr().to_string())
    }

    pub fn remote(&self) -> Option<String> {
        self.request.connection_info().realip_remote_addr().map(|addr| addr.to_string())
    }

    pub fn host(&self) -> Option<String> {
        Some(self.request.connection_info().host().to_string())
    }

    pub fn content_type(&self) -> &str {
        self.request.content_type()
    }

    pub fn header(&self,
                  key: impl Into<HeaderName>, ) -> Option<&HeaderValue> {
        self.request.headers().get(key.into())
    }

    pub fn param(&self, key: &str) -> HttpResult<&str> {
        self.request.match_info().get(key).ok_or(http_err!(ErrorCode::NotFound, "missing parameter"))
    }

    pub fn query<T: DeserializeOwned>(&self) -> HttpResult<T> {
        let query = self.request.query_string();
        serde_qs::from_str(query).map_err(into_http_err!(ErrorCode::InvalidParam, "failed to parse query"))
    }

    pub fn take_body(&mut self) -> Payload {
        if self.payload.is_some() {
            self.payload.take().unwrap()
        } else {
            Payload::None
        }
    }

    pub async fn body_string(&mut self) -> HttpResult<String> {
        let content = self.body_bytes().await?;
        std::str::from_utf8(content.as_slice()).map_err(into_http_err!(ErrorCode::InvalidData, "Not a utf8 format string")).map(|s| s.to_string())
    }

    pub async fn body_bytes(&mut self) -> HttpResult<Vec<u8>> {
        let mut body = self.take_body();
        let mut buf = web::BytesMut::new();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(into_http_err!(ErrorCode::ConnectFailed, "failed to read body"))?;
            buf.extend_from_slice(&chunk);
        }
        Ok(buf.to_vec())
    }

    pub async fn body_json<T: DeserializeOwned>(&mut self) -> HttpResult<T> {
        let body = self.body_string().await?;
        let json = serde_json::from_str(&body).map_err(|e| {
            http_err!(ErrorCode::InvalidData, "parse data failed {}", e)
        })?;
        Ok(json)
    }

    pub async fn body_form<T: DeserializeOwned>(&mut self) -> HttpResult<T> {
        let body = self.body_string().await?;
        serde_qs::from_str(&body).map_err(into_http_err!(ErrorCode::InvalidData, "parse data failed"))
    }
}

pub struct Response {
    pub(crate) resp: Option<HttpResponse>,
}

impl Response {
    pub fn new(status: StatusCode) -> Self {
        Self {
            resp: Some(HttpResponse::new(status))
        }
    }

    pub fn status(&self) -> StatusCode {
        self.resp.as_ref().unwrap().status()
    }

    pub fn set_status(&mut self, status: StatusCode) {
        *self.resp.as_mut().unwrap().status_mut() = status;
    }

    pub fn len(&self) -> Option<usize> {
        match self.resp.as_ref().unwrap().body().size() {
            BodySize::None => {
                Some(0)
            }
            BodySize::Sized(len) => {
                Some(len as usize)
            }
            BodySize::Stream => {
                Some(0)
            }
        }
    }
    pub fn is_empty(&self) -> Option<bool> {
        self.len().map(|len| len == 0)
    }

    pub fn set_body<B: MessageBody + 'static>(&mut self, body: B) {
        self.resp = Some(self.resp.take().unwrap().set_body(BoxBody::new(body)));
    }

    pub fn insert_header(&mut self, name: HeaderName, value: HeaderValue) {
        self.resp.as_mut().unwrap().headers_mut().insert(name, value);

    }

    pub fn set_content_type(&mut self, content_type: &str) -> HttpResult<()> {
        self.insert_header(HeaderName::from_static("Content-Type"), HeaderValue::from_str(content_type)
            .map_err(into_http_err!(ErrorCode::InvalidParam, "invalid content type"))?);
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
pub trait Endpoint<State: Clone + Send + Sync + 'static>: Send + Sync + 'static {
    async fn call(&self, req: Request<State>) -> HttpResult<Response>;
}

#[async_trait::async_trait(?Send)]
impl<State, F, Fut> Endpoint<State> for F
    where
        State: Clone + Send + Sync + 'static,
        F: 'static + Send + Clone + Sync + Fn(Request<State>) -> Fut,
        Fut: Future<Output = HttpResult<Response>> + 'static,
{
    async fn call(&self, req: Request<State>) -> HttpResult<Response> {
        let fut = (self)(req);
        fut.await
    }
}

#[derive(Clone)]
pub struct EndpointHandler<State: Clone + Send + Sync + 'static> {
    ep: Pin<Arc<dyn Endpoint<State>>>,
    state: State,
}

impl<State: Clone + Send + Sync + 'static> EndpointHandler<State> {
    pub fn new(state: State, ep: impl Endpoint<State>) -> Self {
        Self {
            ep: Arc::pin(ep),
            state,
        }
    }
}

impl<State> Service<ServiceRequest> for EndpointHandler<State> where State: 'static + Clone + Send + Sync {
    type Response = ServiceResponse;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::always_ready!();

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let ep = self.ep.clone();
        let state = self.state.clone();
        let fut = async move {
            let (http_req, payload) = req.into_parts();
            let req = Request {
                state,
                request: http_req.clone(),
                payload: Some(payload),
            };

            let res = ep.call(req).await.map_err(|e| {
                let e: Box<dyn std::error::Error + 'static> = Box::new(e);
                Self::Error::from(e)
            })?;

            Ok(ServiceResponse::new(http_req, res.resp.unwrap()))
        };
        Box::pin(fut)
    }
}
