#![allow(unused)]

use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::errors::{Error, ErrorCode, Result};
pub use json::*;
pub use reqwest::*;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

pub async fn http_post_request(url: &str, param: Vec<u8>, content_type: Option<&str>) -> Result<(Vec<u8>, Option<String>)> {
    let mut request_builder = reqwest::Client::new().post(url);
    if content_type.is_some() {
        request_builder = request_builder.header(CONTENT_TYPE, content_type.unwrap());
    }
    // req.set_body(param);
    let mut resp = request_builder.body(param).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let header = resp.headers().get(CONTENT_TYPE);
    let header = if header.is_some() {
        Some(header.unwrap().to_str().map_err(|err| {
            let msg = format!("invalid content-type {}", err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::InvalidParam, msg)
        })?.to_string())
    } else {
        None
    };
    let data = resp.bytes().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })?;
    Ok((data.to_vec(), header))
}

pub async fn http_post_request2<T: for<'de> Deserialize<'de>>(url: &str, param: Vec<u8>, content_type: Option<&str>) -> Result<T> {
    let mut request_builder = reqwest::Client::new().post(url);
    if content_type.is_some() {
        request_builder = request_builder.header(CONTENT_TYPE, content_type.unwrap());
    }
    // req.set_body(param);
    let mut resp = request_builder.body(param).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let data = resp.json().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })?;
    Ok(data)
}

pub async fn http_post_request3<T: for<'de> Deserialize<'de>, P: Serialize>(url: &str, param: &P) -> Result<T> {
    let mut resp = reqwest::Client::new().post(url).json(param).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    resp.json().await.map_err(|err| {
        let msg = format!("recv error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })
}

pub async fn http_get_request2<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T> {
    let resp = reqwest::Client::new().get(url).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    resp.json().await.map_err(|err| {
        let msg = format!("recv error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })
}


pub async fn http_get_request(url: &str) -> Result<(Vec<u8>, Option<String>)> {
    let resp = reqwest::Client::new().get(url).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let header = resp.headers().get(CONTENT_TYPE);
    let header = if header.is_some() {
        Some(header.unwrap().to_str().map_err(|err| {
            let msg = format!("invalid content-type {}", err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::InvalidParam, msg)
        })?.to_string())
    } else {
        None
    };
    let data = resp.bytes().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })?;
    Ok((data.to_vec(), header))
}

pub async fn http_get_request3(url: &str) -> Result<Response> {
    reqwest::Client::new().get(url).send().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })
}

pub async fn http_request(req: Request) -> Result<Response> {
    let url = req.url().to_string();
    reqwest::Client::new().execute(req).await.map_err(|err| {
        let msg = format!("http connect error! url={} err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })
}

pub async fn http_post_json(url: &str, param: JsonValue) -> Result<JsonValue> {
    let resp = reqwest::Client::new()
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .body(param.to_string())
        .send().await.map_err(|err| {
        let msg = format!("http connect error! url={} err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let resp_str = resp.text().await.map_err(|err| {
        let msg = format!("recv error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })?;
    json::parse(resp_str.as_str()).map_err(|err| {
        let msg = format!("parse {} error! err={}", resp_str.as_str(), err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })
}


pub async fn http_post_json2<T: for<'de> Deserialize<'de>>(url: &str, param: JsonValue) -> Result<T> {
    let resp = reqwest::Client::new().post(url)
        .header(CONTENT_TYPE, "application/json")
        .body(param.to_string())
        .send().await.map_err(|err| {
        let msg = format!("http connect error! url={} err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    resp.json().await.map_err(|err| {
        let msg = format!("recv error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
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

    pub async fn get_json<T: for<'de> Deserialize<'de>>(&self, uri: &str) -> Result<T> {
        let mut resp = self.client.get(self.get_url(uri).as_str()).send().await.map_err(|err| {
            let msg = format!("http connect error! url={}, err={}", self.get_url(uri), err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::ConnectFailed, msg)
        })?;

        resp.json().await.map_err(|err| {
            let msg = format!("recv error! err={}", err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::InvalidData, msg)
        })
    }

    pub async fn get(&self, uri: &str) -> Result<(Vec<u8>, Option<String>)> {
        let mut resp = self.client.get(self.get_url(uri).as_str()).send().await.map_err(|err| {
            let msg = format!("http connect error! url={}, err={}", self.get_url(uri), err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::ConnectFailed, msg)
        })?;

        let header = resp.headers().get(CONTENT_TYPE);
        let header = if header.is_some() {
            Some(header.unwrap().to_str().map_err(|err| {
                let msg = format!("invalid content-type {}", err);
                log::error!("{}", msg.as_str());
                Error::new(ErrorCode::InvalidParam, msg)
            })?.to_string())
        } else {
            None
        };
        let data = resp.bytes().await.map_err(|err| {
            let msg = format!("recv body error! err={}", err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::InvalidData, msg)
        })?;
        Ok((data.to_vec(), header))
    }

    pub async fn post_json<T: for<'de> Deserialize<'de>, P: Serialize>(&self, uri: &str, param: &P) -> Result<T> {
        let mut resp = self.client.post(self.get_url(uri)).json(param).send().await.map_err(|err| {
            let msg = format!("http connect error! url={}, err={}", self.get_url(uri), err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::ConnectFailed, msg)
        })?;

        resp.json().await.map_err(|err| {
            let msg = format!("recv error! err={}", err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::InvalidData, msg)
        })
    }

    pub async fn post(&self, uri: &str, param: Vec<u8>, content_type: Option<&str>) -> Result<(Vec<u8>, Option<String>)> {
        let mut request_builder = self.client.post(self.get_url(uri));
        if content_type.is_some() {
            request_builder = request_builder.header(CONTENT_TYPE, content_type.unwrap());
        }
        // req.set_body(param);
        let mut resp = request_builder.body(param).send().await.map_err(|err| {
            let msg = format!("http connect error! host={}, err={}", self.get_url(uri), err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::ConnectFailed, msg)
        })?;

        let header = resp.headers().get(CONTENT_TYPE);
        let header = if header.is_some() {
            Some(header.unwrap().to_str().map_err(|err| {
                let msg = format!("invalid content-type {}", err);
                log::error!("{}", msg.as_str());
                Error::new(ErrorCode::InvalidParam, msg)
            })?.to_string())
        } else {
            None
        };

        let data = resp.bytes().await.map_err(|err| {
            let msg = format!("recv body error! err={}", err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::InvalidData, msg)
        })?;
        Ok((data.to_vec(), header))
    }
}

pub struct HttpClientBuilder {
    base_url: Option<String>,
    headers: HeaderMap<HeaderValue>,
    http_keep_alive: bool,
    tcp_no_delay: bool,
    timeout: Option<Duration>,
    max_connections_per_host: usize,
    verify_tls: bool,
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self {
            base_url: None,
            headers: Default::default(),
            http_keep_alive: true,
            tcp_no_delay: false,
            timeout: Some(Duration::from_secs(60)),
            max_connections_per_host: 50,
            verify_tls: true,
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
    ) -> Result<Self> {
        self.headers
            .insert(name.into(), value.into());
        Ok(self)
    }

    pub fn set_http_keep_alive(mut self, keep_alive: bool) -> Self {
        self.http_keep_alive = keep_alive;
        self
    }

    pub fn set_tcp_no_delay(mut self, no_delay: bool) -> Self {
        self.tcp_no_delay = no_delay;
        self
    }

    pub fn set_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn set_max_connections_per_host(mut self, max_connections_per_host: usize) -> Self {
        self.max_connections_per_host = max_connections_per_host;
        self
    }

    pub fn set_verify_tls(mut self, verify_tls: bool) -> Self {
        self.verify_tls = verify_tls;
        self
    }

    pub fn build(self) -> HttpClient {
        let mut config = reqwest::ClientBuilder::new()
            .pool_max_idle_per_host(self.max_connections_per_host)
            .http2_keep_alive_while_idle(self.http_keep_alive)
            .danger_accept_invalid_certs(self.verify_tls)
            .default_headers(self.headers);
        if self.timeout.is_some() {
            config = config.connect_timeout(self.timeout.unwrap())
        }

        HttpClient {
            client: config.build().unwrap(),
            base_url: self.base_url,
        }
    }
}
