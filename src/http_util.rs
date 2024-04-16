#![allow(unused)]

mod json {
    pub use json::*;
}

pub use reqwest::*;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use ::json::JsonValue;
use serde::{Deserialize, Serialize};
use crate::errors::{HttpError, ErrorCode, HttpResult};
use reqwest::dns::Resolve;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

pub async fn http_post_request(url: &str, param: Vec<u8>, content_type: Option<&str>) -> HttpResult<(Vec<u8>, Option<String>)> {
    let mut request_builder = reqwest::Client::builder().no_proxy().build().unwrap().post(url);
    if content_type.is_some() {
        request_builder = request_builder.header(CONTENT_TYPE, content_type.unwrap());
    }
    // req.set_body(param);
    let mut resp = request_builder.body(param).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::ConnectFailed, msg)
    })?;

    let header = resp.headers().get(CONTENT_TYPE);
    let header = if header.is_some() {
        Some(header.unwrap().to_str().map_err(|err| {
            let msg = format!("invalid content-type {}", err);
            log::error!("{}", msg.as_str());
            HttpError::new(ErrorCode::InvalidParam, msg)
        })?.to_string())
    } else {
        None
    };
    let data = resp.bytes().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::InvalidData, msg)
    })?;
    Ok((data.to_vec(), header))
}

pub async fn http_post_request2<T: for<'de> Deserialize<'de>>(url: &str, param: Vec<u8>, content_type: Option<&str>) -> HttpResult<T> {
    let mut request_builder = reqwest::Client::builder().no_proxy().build().unwrap().post(url);
    if content_type.is_some() {
        request_builder = request_builder.header(CONTENT_TYPE, content_type.unwrap());
    }
    // req.set_body(param);
    let mut resp = request_builder.body(param).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::ConnectFailed, msg)
    })?;

    let data = resp.json().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::InvalidData, msg)
    })?;
    Ok(data)
}

pub async fn http_post_request3<T: for<'de> Deserialize<'de>, P: Serialize>(url: &str, param: &P) -> HttpResult<T> {
    let mut resp = reqwest::Client::builder().no_proxy().build().unwrap().post(url).json(param).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::ConnectFailed, msg)
    })?;

    resp.json().await.map_err(|err| {
        let msg = format!("recv error! err={}", err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::InvalidData, msg)
    })
}

pub async fn http_get_request2<T: for<'de> Deserialize<'de>>(url: &str) -> HttpResult<T> {
    let resp = reqwest::Client::builder().no_proxy().build().unwrap().get(url).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::ConnectFailed, msg)
    })?;

    resp.json().await.map_err(|err| {
        let msg = format!("recv error! err={}", err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::InvalidData, msg)
    })
}


pub async fn http_get_request(url: &str) -> HttpResult<(Vec<u8>, Option<String>)> {
    let resp = reqwest::Client::builder().no_proxy().build().unwrap().get(url).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::ConnectFailed, msg)
    })?;

    let header = resp.headers().get(CONTENT_TYPE);
    let header = if header.is_some() {
        Some(header.unwrap().to_str().map_err(|err| {
            let msg = format!("invalid content-type {}", err);
            log::error!("{}", msg.as_str());
            HttpError::new(ErrorCode::InvalidParam, msg)
        })?.to_string())
    } else {
        None
    };
    let data = resp.bytes().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::InvalidData, msg)
    })?;
    Ok((data.to_vec(), header))
}

pub async fn http_get_request3(url: &str) -> HttpResult<Response> {
    reqwest::Client::builder().no_proxy().build().unwrap().get(url).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::ConnectFailed, msg)
    })
}

pub async fn http_request(req: Request) -> HttpResult<Response> {
    let url = req.url().to_string();
    reqwest::Client::builder().no_proxy().build().unwrap().execute(req).await.map_err(|err| {
        let msg = format!("http connect error! url={} err={}", url, err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::ConnectFailed, msg)
    })
}

pub async fn http_post_json(url: &str, param: JsonValue) -> HttpResult<JsonValue> {
    let resp = reqwest::Client::builder().no_proxy().build().unwrap()
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .body(param.to_string())
        .send().await.map_err(|err| {
        let msg = format!("http connect error! url={} err={}", url, err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::ConnectFailed, msg)
    })?;

    let resp_str = resp.text().await.map_err(|err| {
        let msg = format!("recv error! err={}", err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::InvalidData, msg)
    })?;
    json::parse(resp_str.as_str()).map_err(|err| {
        let msg = format!("parse {} error! err={}", resp_str.as_str(), err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::InvalidData, msg)
    })
}


pub async fn http_post_json2<T: for<'de> Deserialize<'de>>(url: &str, param: JsonValue) -> HttpResult<T> {
    let resp = reqwest::Client::builder().no_proxy().build().unwrap().post(url)
        .header(CONTENT_TYPE, "application/json")
        .body(param.to_string())
        .send().await.map_err(|err| {
        let msg = format!("http connect error! url={} err={}", url, err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::ConnectFailed, msg)
    })?;

    resp.json().await.map_err(|err| {
        let msg = format!("recv error! err={}", err);
        log::error!("{}", msg.as_str());
        HttpError::new(ErrorCode::InvalidData, msg)
    })
}

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    base_url: Option<String>,
}

impl Debug for HttpClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "HttpClient")
    }
}

impl HttpClient {
    pub fn new(max_connections: usize, base_url: Option<&str>) -> Self {
        let client = reqwest::ClientBuilder::new()
            .connect_timeout(Duration::from_secs(30))
            .http2_keep_alive_while_idle(true)
            .pool_max_idle_per_host(max_connections)
            .no_proxy()
            .build().unwrap();

        let base_url = if base_url.is_some() {
            let base_url = base_url.unwrap();
            let base_url = if base_url.ends_with("/") {
                base_url.to_string()
            } else {
                format!("{}/", base_url)
            };
            Some(base_url)
        } else {
            base_url.map(|v| v.to_string())
        };

        Self {
            client,
            base_url,
        }
    }

    pub fn new_with_no_cert_verify(max_connections: usize, base_url: Option<&str>) -> Self {
        let client = reqwest::ClientBuilder::new()
            .connect_timeout(Duration::from_secs(30))
            .http2_keep_alive_while_idle(true)
            .use_rustls_tls()
            .pool_max_idle_per_host(max_connections)
            .danger_accept_invalid_certs(true)
            .no_proxy()
            .build().unwrap();

        let base_url = if base_url.is_some() {
            let base_url = base_url.unwrap();
            let base_url = if base_url.ends_with("/") {
                base_url.to_string()
            } else {
                format!("{}/", base_url)
            };
            Some(base_url)
        } else {
            base_url.map(|v| v.to_string())
        };

        Self {
            client,
            base_url,
        }
    }

    fn get_url(&self, uri: &str) -> String {
        if self.base_url.is_some() {
            format!("{}{}", self.base_url.as_ref().unwrap(), uri)
        } else {
            uri.to_string()
        }
    }

    pub async fn get_json<T: for<'de> Deserialize<'de>>(&self, uri: &str) -> HttpResult<T> {
        let mut resp = self.client.get(self.get_url(uri).as_str()).send().await.map_err(|err| {
            let msg = format!("http connect error! url={}, err={}", self.get_url(uri), err);
            log::error!("{}", msg.as_str());
            HttpError::new(ErrorCode::ConnectFailed, msg)
        })?;

        resp.json().await.map_err(|err| {
            let msg = format!("recv error! err={}", err);
            log::error!("{}", msg.as_str());
            HttpError::new(ErrorCode::InvalidData, msg)
        })
    }

    pub async fn get(&self, uri: &str) -> HttpResult<(Vec<u8>, Option<String>)> {
        let mut resp = self.client.get(self.get_url(uri).as_str()).send().await.map_err(|err| {
            let msg = format!("http connect error! url={}, err={}", self.get_url(uri), err);
            log::error!("{}", msg.as_str());
            HttpError::new(ErrorCode::ConnectFailed, msg)
        })?;

        let header = resp.headers().get(CONTENT_TYPE);
        let header = if header.is_some() {
            Some(header.unwrap().to_str().map_err(|err| {
                let msg = format!("invalid content-type {}", err);
                log::error!("{}", msg.as_str());
                HttpError::new(ErrorCode::InvalidParam, msg)
            })?.to_string())
        } else {
            None
        };
        let data = resp.bytes().await.map_err(|err| {
            let msg = format!("recv body error! err={}", err);
            log::error!("{}", msg.as_str());
            HttpError::new(ErrorCode::InvalidData, msg)
        })?;
        Ok((data.to_vec(), header))
    }

    pub async fn post_json<T: for<'de> Deserialize<'de>, P: Serialize>(&self, uri: &str, param: &P) -> HttpResult<T> {
        let mut resp = self.client.post(self.get_url(uri)).json(param).send().await.map_err(|err| {
            let msg = format!("http connect error! url={}, err={}", self.get_url(uri), err);
            log::error!("{}", msg.as_str());
            HttpError::new(ErrorCode::ConnectFailed, msg)
        })?;

        resp.json().await.map_err(|err| {
            let msg = format!("recv error! err={}", err);
            log::error!("{}", msg.as_str());
            HttpError::new(ErrorCode::InvalidData, msg)
        })
    }

    pub async fn post(&self, uri: &str, param: Vec<u8>, content_type: Option<&str>) -> HttpResult<(Vec<u8>, Option<String>)> {
        let mut request_builder = self.client.post(self.get_url(uri));
        if content_type.is_some() {
            request_builder = request_builder.header(CONTENT_TYPE, content_type.unwrap());
        }
        // req.set_body(param);
        let mut resp = request_builder.body(param).send().await.map_err(|err| {
            let msg = format!("http connect error! host={}, err={}", self.get_url(uri), err);
            log::error!("{}", msg.as_str());
            HttpError::new(ErrorCode::ConnectFailed, msg)
        })?;

        let header = resp.headers().get(CONTENT_TYPE);
        let header = if header.is_some() {
            Some(header.unwrap().to_str().map_err(|err| {
                let msg = format!("invalid content-type {}", err);
                log::error!("{}", msg.as_str());
                HttpError::new(ErrorCode::InvalidParam, msg)
            })?.to_string())
        } else {
            None
        };

        let data = resp.bytes().await.map_err(|err| {
            let msg = format!("recv body error! err={}", err);
            log::error!("{}", msg.as_str());
            HttpError::new(ErrorCode::InvalidData, msg)
        })?;
        Ok((data.to_vec(), header))
    }
}

pub struct HttpClientBuilder {
    base_url: Option<String>,
    builder: ClientBuilder,
    headers: HeaderMap,
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self {
            base_url: None,
            builder: ClientBuilder::new(),
            headers: Default::default(),
        }
    }
}

impl HttpClientBuilder {
    pub fn set_base_url(mut self, base_url: &str) -> Self {
        let base_url = if base_url.ends_with("/") {
            base_url.to_string()
        } else {
            format!("{}/", base_url)
        };
        self.base_url = Some(base_url);
        self
    }
    pub fn add_header(
        mut self,
        name: impl Into<HeaderName>,
        value: impl Into<HeaderValue>,
    ) -> HttpResult<Self> {
        self.headers
            .insert(name.into(), value.into());
        Ok(self)
    }

    pub fn set_http_keep_alive(mut self, keep_alive: bool) -> Self {
        self.builder = self.builder.http2_keep_alive_while_idle(keep_alive);
        self
    }

    pub fn set_tcp_no_delay(mut self, no_delay: bool) -> Self {
        self.builder = self.builder.tcp_nodelay(no_delay);
        self
    }

    pub fn set_timeout(mut self, timeout: Duration) -> Self {
        self.builder = self.builder.connect_timeout(timeout);
        self
    }

    pub fn set_max_connections_per_host(mut self, max_connections_per_host: usize) -> Self {
        self.builder = self.builder.pool_max_idle_per_host(max_connections_per_host);
        self
    }

    pub fn set_verify_tls(mut self, verify_tls: bool) -> Self {
        self.builder = self.builder.danger_accept_invalid_certs(!verify_tls);
        self
    }

    pub fn set_auto_sys_proxy(mut self, proxy: bool) -> Self {
        if !proxy {
            self.builder = self.builder.no_proxy();
        }
        self
    }

    pub fn add_root_certificate(mut self, cert: Certificate) -> Self {
        self.builder = self.builder.add_root_certificate(cert);
        self
    }

    pub fn tls_built_in_root_certs(mut self, tls_built_in_root_certs: bool) -> Self {
        self.builder = self.builder.tls_built_in_root_certs(tls_built_in_root_certs);
        self
    }

    pub fn identity(mut self, identity: Identity) -> Self {
        self.builder = self.builder.identity(identity);
        self
    }

    pub fn tls_sni(mut self, tls_sni: bool) -> Self {
        self.builder = self.builder.tls_sni(tls_sni);
        self
    }

    pub fn min_tls_version(mut self, version: tls::Version) -> Self {
        self.builder = self.builder.min_tls_version(version);
        self
    }

    pub fn max_tls_version(mut self, version: tls::Version) -> Self {
        self.builder = self.builder.max_tls_version(version);
        self
    }

    pub fn https_only(mut self, enabled: bool) -> Self {
        self.builder = self.builder.https_only(enabled);
        self
    }

    pub fn resolve(mut self, domain: &str, addr: SocketAddr) -> Self {
        self.builder = self.builder.resolve(domain, addr);
        self
    }

    pub fn resolve_to_addrs(mut self, domain: &str, addrs: &[SocketAddr]) -> Self {
        self.builder = self.builder.resolve_to_addrs(domain, addrs);
        self
    }

    pub fn dns_resolver<R: Resolve + 'static>(mut self, resolver: Arc<R>) -> Self {
        self.builder = self.builder.dns_resolver(resolver);
        self
    }

    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.builder = self.builder.proxy(proxy);
        self
    }

    pub fn build(mut self) -> HttpClient {
        let mut config = self.builder.default_headers(self.headers);

        HttpClient {
            client: config.build().unwrap(),
            base_url: self.base_url,
        }
    }
}
