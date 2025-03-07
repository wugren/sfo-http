use std::fmt::Debug;
use std::future::Future;
use std::path::Path;
use http::{HeaderName, HeaderValue, StatusCode};
use http::header::COOKIE;
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::errors::HttpResult;

#[async_trait::async_trait(?Send)]
pub trait Request: 'static {
    fn peer_addr(&self) -> Option<String>;
    fn local_addr(&self) -> Option<String>;
    fn remote(&self) -> Option<String>;
    fn host(&self) -> Option<String>;
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

pub trait Response: 'static {
    fn from_result<T: Serialize, C: Debug + Copy + Sync + Send + 'static + Into<u16>>(ret: sfo_result::Result<T, C>) -> Self;
    fn new(status: StatusCode) -> Self;
    fn insert_header(&mut self, name: HeaderName, value: HeaderValue);
    fn set_content_type(&mut self, content_type: &str) -> HttpResult<()>;
    fn set_body(&mut self, body: Vec<u8>);
}

#[async_trait::async_trait(?Send)]
pub trait Endpoint<Req: Request, Resp: Response>: Send + Sync + 'static {
    async fn call(&self, req: Req) -> HttpResult<Resp>;
}

#[async_trait::async_trait(?Send)]
impl<Req, Resp, F, Fut> Endpoint<Req, Resp> for F
where
    Req: Request,
    Resp: Response,
    F: 'static + Send + Clone + Sync + Fn(Req) -> Fut,
    Fut: Future<Output = HttpResult<Resp>> + 'static,
{
    async fn call(&self, req: Req) -> HttpResult<Resp> {
        let fut = (self)(req);
        fut.await
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE
}
pub trait HttpServer< Req: Request, Resp: Response> {
    fn serve(&mut self, path: &str, method: HttpMethod, ep: impl Endpoint<Req, Resp>);
    fn serve_dir(&mut self, path: &str, dir: impl AsRef<Path>) -> HttpResult<()>;
    fn serve_file(&mut self, path: &str, file: impl AsRef<Path>) -> HttpResult<()>;
}
