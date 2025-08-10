use std::fmt::Debug;
use std::future::Future;
use std::path::Path;
use http::{HeaderName, HeaderValue, Method, StatusCode};
use http::header::COOKIE;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncRead;
use crate::errors::HttpResult;

#[derive(Serialize, Deserialize)]
pub struct HttpServerResult<T>
{
    pub err: u16,
    pub msg: String,
    pub result: Option<T>
}

#[async_trait::async_trait]
pub trait Request: 'static + Send {
    fn peer_addr(&self) -> Option<String>;
    fn local_addr(&self) -> Option<String>;
    fn remote(&self) -> Option<String>;
    fn host(&self) -> Option<String>;
    fn path(&self) -> &str;
    fn method(&self) -> Method;
    fn content_type(&self) -> Option<String>;
    fn header(&self,
              key: impl Into<HeaderName>, ) -> Option<HeaderValue>;
    fn header_all(&self, key: impl Into<HeaderName>) -> Vec<HeaderValue>;
    fn param(&self, key: &str) -> HttpResult<&str>;
    fn query<T: DeserializeOwned>(&self) -> HttpResult<T>;
    async fn body_string(&mut self) -> HttpResult<String>;
    async fn body_bytes(&mut self) -> HttpResult<Vec<u8>>;
    async fn body_json<T: DeserializeOwned>(&mut self) -> HttpResult<T>;
    async fn body_form<T: DeserializeOwned>(&mut self) -> HttpResult<T>;
    fn get_cookie(&self, cookie_name: &str) -> Option<String> {
        let cookie = self.header_all(COOKIE);
        if cookie.is_empty() {
            return None;
        }
        let cookie_str = match cookie.last().unwrap().to_str() {
            Ok(v) => v,
            Err(_) => return None,
        };
        let cookie_list: Vec<_> = cookie_str.split(";").collect();
        let cookie_list: Vec<(String, String)> = cookie_list.into_iter().map(|v| {
            let cookie_list: Vec<_> = v.split("=").collect();
            cookie_list
        }).filter(|v| v.len() == 2).map(|v| (v[0].trim().to_string(), v[1].trim().to_string())).collect();

        for (name, value) in cookie_list.into_iter() {
            if name.as_str() == cookie_name {
                return Some(value);
            }
        }

        None
    }
}

pub trait Response: 'static + Send {
    fn from_result<T: Serialize, C: Debug + Copy + Sync + Send + 'static + Into<u16>>(ret: sfo_result::Result<T, C>) -> Self;
    fn new(status: StatusCode) -> Self;
    fn insert_header(&mut self, name: HeaderName, value: HeaderValue);
    fn set_content_type(&mut self, content_type: &str) -> HttpResult<()>;
    fn set_body(&mut self, body: Vec<u8>);
    fn set_body_read<R: AsyncRead + Send + Unpin + 'static>(&mut self, reader: R);
}

#[async_trait::async_trait]
pub trait Endpoint<Req: Request, Resp: Response>: Send + Sync + 'static {
    async fn call(&self, req: Req) -> HttpResult<Resp>;
}

#[async_trait::async_trait]
impl<Req, Resp, F, Fut> Endpoint<Req, Resp> for F
where
    Req: Request,
    Resp: Response,
    F: 'static + Send + Clone + Sync + Fn(Req) -> Fut,
    Fut: Future<Output = HttpResult<Resp>> + Send + 'static,
{
    async fn call(&self, req: Req) -> HttpResult<Resp> {
        let fut = (self)(req);
        fut.await
    }
}

pub type HttpMethod = http::Method;

pub trait HttpServer< Req: Request, Resp: Response> {
    fn serve(&mut self, path: &str, method: HttpMethod, ep: impl Endpoint<Req, Resp>);
    fn serve_dir(&mut self, path: &str, dir: impl AsRef<Path>) -> HttpResult<()>;
    fn serve_file(&mut self, path: &str, file: impl AsRef<Path>) -> HttpResult<()>;
}

#[derive(Debug, Clone)]
pub struct HttpServerConfig {
    pub(crate) server_addr: String,
    pub(crate) port: u16,
    pub(crate) allow_origins: Vec<String>,
    pub(crate) allow_methods: Vec<String>,
    pub(crate) allow_headers: Vec<String>,
    pub(crate) expose_headers: Vec<String>,
    pub(crate) max_age: usize,
    pub(crate) support_credentials: bool,
}

impl HttpServerConfig {
    pub fn new(server_addr: impl Into<String>, port: u16) -> Self {
        Self {
            server_addr: server_addr.into(),
            port,
            allow_origins: vec![],
            allow_methods: vec![],
            allow_headers: vec![],
            expose_headers: vec![],
            max_age: 3600,
            support_credentials: false,
        }
    }

    pub fn allow_origins(mut self, origin: Vec<String>) -> Self {
        self.allow_origins = origin;
        self
    }

    pub fn allow_any_origin(mut self) -> Self {
        self.allow_origins = vec!["*".to_string()];
        self
    }

    pub fn allow_methods(mut self, methods: Vec<String>) -> Self {
        self.allow_methods = methods;
        self
    }

    pub fn allow_any_methods(mut self) -> Self {
        self.allow_methods = vec!["*".to_string()];
        self
    }

    pub fn allow_headers(mut self, headers: Vec<String>) -> Self {
        self.allow_headers = headers;
        self
    }

    pub fn allow_any_header(mut self) -> Self {
        self.allow_headers = vec!["*".to_string()];
        self
    }

    pub fn expose_headers(mut self, headers: Vec<String>) -> Self {
        self.expose_headers = headers;
        self
    }

    pub fn expose_any_header(mut self) -> Self {
        self.expose_headers = vec!["*".to_string()];
        self
    }

    pub fn max_age(mut self, age: usize) -> Self {
        self.max_age = age;
        self
    }

    pub fn support_credentials(mut self, support: bool) -> Self {
        self.support_credentials = support;
        self
    }
}
