use std::ffi::OsStr;
use std::fmt::Debug;
use std::future::Future;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};
use actix_files::NamedFile;
use actix_web::{FromRequest, Handler, HttpMessage, HttpRequest, HttpResponse, Responder, web};
use actix_web::body::{BodySize, BodyStream, BoxBody, MessageBody};
use actix_web::dev::{Payload, Service, ServiceRequest, ServiceResponse};
use actix_web::error::PayloadError;
use actix_web::http::{Method, StatusCode, Version};
use actix_web::web::Bytes;
use async_trait::async_trait;
use futures_util::future::LocalBoxFuture;
use futures_util::{Stream, StreamExt, TryStreamExt};
use futures_util::stream::Next;
use http::{HeaderName, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncRead;
use crate::errors::{ErrorCode, http_err, HttpError, HttpResult, into_http_err};
use crate::http_server::{Endpoint, Request, Response};

pub(crate) struct UnsafeObject<T> {
    object: T,
}
impl<T> UnsafeObject<T> {
    fn new(object: T) -> Self {
        Self {
            object,
        }
    }
}

impl<T> Deref for UnsafeObject<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

impl<T> DerefMut for UnsafeObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.object
    }
}


unsafe impl<T> Sync for UnsafeObject<T> {}
unsafe impl<T> Send for UnsafeObject<T> {}

type UnsafeHttpResponse = UnsafeObject<HttpResponse>;
type UnsafeHttpRequest = UnsafeObject<HttpRequest>;

pub struct UnsafePayload {
    payload: Payload,
}

impl UnsafePayload {
    fn new(payload: Payload) -> Self {
        Self {
            payload,
        }
    }
}

impl<'a> Stream for UnsafePayload {
    type Item = HttpResult<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.payload.poll_next_unpin(cx) {
            Poll::Ready(ret) => {
                Poll::Ready(ret.map(|ret| ret.map_err(into_http_err!(ErrorCode::IOError))))
            }
            Poll::Pending => {
                Poll::Pending
            }
        }
    }
}

unsafe impl Sync for UnsafePayload {}
unsafe impl Send for UnsafePayload {}

pub struct ActixRequest {
    request: UnsafeHttpRequest,
    payload: Option<UnsafePayload>,
}

impl ActixRequest {
    pub fn request(&self) -> &HttpRequest {
        &self.request
    }

    pub fn method(&self) -> Method {
        self.request.method().clone()
    }

    pub fn url(&self) -> &actix_web::dev::Url {
        self.request.match_info().get_ref()
    }

    pub fn version(&self) -> Option<Version> {
        Some(self.request.version())
    }

    pub fn take_body(&mut self) -> UnsafePayload {
        if self.payload.is_some() {
            self.payload.take().unwrap()
        } else {
            UnsafePayload::new(Payload::None)
        }
    }

    fn payload(&mut self) -> NextFuture {
        let next = self.payload.as_mut().unwrap().next();
        NextFuture::new(next)
    }
}

struct NextFuture<'a> {
    next: Next<'a, UnsafePayload>,
}

unsafe impl<'a> Send for NextFuture<'a> {}

impl<'a> NextFuture<'a> {
    fn new(next: Next<'a, UnsafePayload>) -> Self {
        Self {
            next,
        }
    }
}

impl<'a> Future for NextFuture<'a> {
    type Output = Option<Result<Bytes, HttpError>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        match Pin::new(&mut self.next).poll(cx) {
            Poll::Ready(ret) => {
                match ret {
                    Some(Ok(chunk)) => {
                        Poll::Ready(Some(Ok(chunk)))
                    }
                    Some(Err(e)) => {
                        Poll::Ready(Some(Err(http_err!(ErrorCode::IOError))))
                    }
                    None => {
                        Poll::Ready(None)
                    }
                }
            }
            Poll::Pending => {
                Poll::Pending
            }
        }
    }
}

#[async_trait::async_trait]
impl Request for ActixRequest {
    fn peer_addr(&self) -> Option<String> {
        self.request.peer_addr().map(|addr| addr.to_string())
    }

    fn local_addr(&self) -> Option<String> {
        Some(self.request.app_config().local_addr().to_string())
    }

    fn remote(&self) -> Option<String> {
        self.request.connection_info().realip_remote_addr().map(|addr| addr.to_string())
    }

    fn host(&self) -> Option<String> {
        Some(self.request.connection_info().host().to_string())
    }

    fn path(&self) -> &str {
        self.request.uri().path()
    }

    fn method(&self) -> http::Method {
        match self.request.method() {
            &Method::GET => http::Method::GET,
            &Method::POST => http::Method::POST,
            &Method::PUT => http::Method::PUT,
            &Method::DELETE => http::Method::DELETE,
            &Method::HEAD => http::Method::HEAD,
            &Method::OPTIONS => http::Method::OPTIONS,
            &Method::PATCH => http::Method::PATCH,
            &Method::TRACE => http::Method::TRACE,
            _ => http::Method::TRACE,
        }
    }

    fn content_type(&self) -> Option<String> {
        Some(self.request.content_type().to_string())
    }

    fn header(&self,
                  key: impl Into<HeaderName>, ) -> Option<HeaderValue> {
        self.request.headers().get(key.into().as_str()).map(|v| HeaderValue::try_from(v.as_bytes()).unwrap())
    }

    fn header_all(&self, key: impl Into<HeaderName>) -> Vec<HeaderValue> {
        self.request.headers().get_all(key.into().as_str()).map(|v| HeaderValue::try_from(v.as_bytes()).unwrap()).collect::<Vec<HeaderValue>>()
    }

    fn param(&self, key: &str) -> HttpResult<&str> {
        self.request.match_info().get(key).ok_or(http_err!(ErrorCode::NotFound, "missing parameter"))
    }

    fn query<T: DeserializeOwned>(&self) -> HttpResult<T> {
        let query = self.request.query_string();
        serde_qs::from_str(query).map_err(into_http_err!(ErrorCode::InvalidParam, "failed to parse query"))
    }

    async fn body_string(&mut self) -> HttpResult<String> {
        let content = self.body_bytes().await?;
        std::str::from_utf8(content.as_slice()).map_err(into_http_err!(ErrorCode::InvalidData, "Not a utf8 format string")).map(|s| s.to_string())
    }

    async fn body_bytes(&mut self) -> HttpResult<Vec<u8>> {
        let mut body = self.take_body();
        let mut buf = web::BytesMut::new();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(into_http_err!(ErrorCode::ConnectFailed, "failed to read body"))?;
            buf.extend_from_slice(&chunk);
        }
        Ok(buf.to_vec())
    }

    async fn body_json<T: DeserializeOwned>(&mut self) -> HttpResult<T> {
        let body = self.body_string().await?;
        let json = serde_json::from_str(&body).map_err(|e| {
            http_err!(ErrorCode::InvalidData, "parse data failed {}", e)
        })?;
        Ok(json)
    }

    async fn body_form<T: DeserializeOwned>(&mut self) -> HttpResult<T> {
        let body = self.body_string().await?;
        serde_qs::from_str(&body).map_err(into_http_err!(ErrorCode::InvalidData, "parse data failed"))
    }
}

pub struct ActixResponse {
    pub(crate) resp: Option<UnsafeHttpResponse>,
}

impl ActixResponse {
    pub fn new(status: StatusCode) -> Self {
        Self {
            resp: Some(UnsafeHttpResponse::new(HttpResponse::new(status)))
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
}

impl From<HttpResponse> for ActixResponse {
    fn from(resp: HttpResponse) -> Self {
        Self {
            resp: Some(UnsafeHttpResponse::new(resp))
        }
    }

}

#[derive(Serialize, Deserialize)]
struct HttpJsonResult<T>
{
    pub err: u16,
    pub msg: String,
    pub result: Option<T>
}

impl Response for ActixResponse {
    fn from_result<T: Serialize, C: Debug + Copy + Sync + Send + 'static + Into<u16>>(ret: sfo_result::Result<T, C>) -> Self {
        let result = match ret {
            Ok(data) => {
                HttpJsonResult {
                    err: 0,
                    msg: "".to_string(),
                    result: Some(data)
                }
            },
            Err(err) => {
                let msg = if err.msg().is_empty() {
                    format!("{:?}", err.code())
                } else {
                    err.msg().to_string()
                };
                HttpJsonResult {
                    err: err.code().into(),
                    msg,
                    result: None
                }
            }
        };

        let mut resp = ActixResponse::new(StatusCode::OK);
        resp.set_content_type("application/json");
        resp.set_body(serde_json::to_string(&result).unwrap().as_bytes().to_vec());
        resp
    }

    fn new(status: http::StatusCode) -> Self {
        ActixResponse::new(StatusCode::from_u16(status.as_u16()).unwrap())
    }

    fn insert_header(&mut self, name: HeaderName, value: HeaderValue) {
        self.resp.as_mut().unwrap().headers_mut().append(actix_web::http::header::HeaderName::from_str(name.as_str()).unwrap(), actix_web::http::header::HeaderValue::from_bytes(value.as_bytes()).unwrap());
    }

    fn set_content_type(&mut self, content_type: &str) -> HttpResult<()> {
        self.insert_header(HeaderName::from_static("content-type"), HeaderValue::from_str(content_type)
            .map_err(into_http_err!(ErrorCode::InvalidParam, "invalid content type"))?);
        Ok(())
    }

    fn set_body(&mut self, body: Vec<u8>) {
        let resp = self.resp.take().unwrap();
        self.resp = Some(UnsafeHttpResponse::new(resp.object.set_body(BoxBody::new(body))));
    }

    fn set_body_read<R: AsyncRead + Send + 'static>(&mut self, reader: R) {
        let resp = self.resp.take().unwrap();
        let reader = tokio_util::io::ReaderStream::new(reader);
        self.resp = Some(UnsafeHttpResponse::new(resp.object.set_body(BoxBody::new(BodyStream::new(reader)))));
    }
}

pub(crate) struct ServeDir {
    prefix: String,
    dir: PathBuf,
}

impl ServeDir {
    /// Create a new instance of `ServeDir`.
    pub(crate) fn new(prefix: String, dir: PathBuf) -> Self {
        Self { prefix, dir }
    }
}

#[async_trait::async_trait]
impl Endpoint<ActixRequest, ActixResponse> for ServeDir
{
    async fn call(&self, req: ActixRequest) -> HttpResult<ActixResponse> {
        let path = req.url().path();
        let path = path.strip_prefix(&self.prefix).unwrap();
        let path = path.trim_start_matches('/');
        let mut file_path = self.dir.clone();
        for p in Path::new(path) {
            if p == OsStr::new(".") {
                continue;
            } else if p == OsStr::new("..") {
                file_path.pop();
            } else {
                file_path.push(&p);
            }
        }

        log::info!("Requested file: {:?}", file_path);

        if !file_path.starts_with(&self.dir) {
            log::warn!("Unauthorized attempt to read: {:?}", file_path);
            Ok(ActixResponse::new(StatusCode::FORBIDDEN))
        } else {
            match NamedFile::open_async(file_path.as_path()).await {
                Ok(file) => {
                    let resp = ActixResponse::from(file.into_response(req.request()));
                    Ok(resp)
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    log::warn!("File not found: {:?}", &file_path);
                    Ok(ActixResponse::new(StatusCode::NOT_FOUND))
                },
                Err(e) => Err(http_err!(ErrorCode::IOError, "read file failed {}", e)),
            }
        }
    }
}

pub(crate) struct ServeFile {
    path: PathBuf,
}

impl ServeFile {
    /// Create a new instance of `ServeFile`.
    pub(crate) fn init(path: impl AsRef<Path>) -> HttpResult<Self> {
        let file = path.as_ref().to_owned().canonicalize().map_err(into_http_err!(ErrorCode::IOError, "path {} failed", path.as_ref().to_string_lossy()))?;
        Ok(Self {
            path: PathBuf::from(file),
        })
    }
}

#[async_trait]
impl Endpoint<ActixRequest, ActixResponse> for ServeFile {
    async fn call(&self, req: ActixRequest) -> HttpResult<ActixResponse> {
        match NamedFile::open_async(self.path.as_path()).await {
            Ok(file) => {
                let resp = ActixResponse::from(file.into_response(req.request()));
                Ok(resp)
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::warn!("File not found: {:?}", &self.path);
                Ok(ActixResponse::new(StatusCode::NOT_FOUND))
            },
            Err(e) => Err(http_err!(ErrorCode::IOError, "read file failed {}", e)),
        }
    }
}

pub struct EndpointHandler {
    ep: Pin<Arc<dyn Endpoint<ActixRequest, ActixResponse>>>,
}

impl EndpointHandler {
    pub fn new(ep: impl Endpoint<ActixRequest, ActixResponse>) -> Self {
        Self {
            ep: Arc::pin(ep),
        }
    }
}

impl Clone for EndpointHandler {
    fn clone(&self) -> Self {
        Self {
            ep: self.ep.clone(),
        }
    }
}

impl Service<ServiceRequest> for EndpointHandler {
    type Response = ServiceResponse;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::always_ready!();

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let ep = self.ep.clone();
        let fut = async move {
            let (http_req, payload) = req.into_parts();
            let req = ActixRequest {
                request: UnsafeHttpRequest::new(http_req.clone()),
                payload: Some(UnsafePayload::new(payload)),
            };

            let res = ep.call(req).await.map_err(|e| {
                let e: Box<dyn std::error::Error + 'static> = Box::new(e);
                Self::Error::from(e)
            })?;

            Ok(ServiceResponse::new(http_req, res.resp.unwrap().object))
        };
        Box::pin(fut)
    }
}
