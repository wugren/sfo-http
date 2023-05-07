#![allow(unused)]

use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use std::time::Duration;
use http_client::http_types;
use rustls::{Certificate, RootCertStore, ServerCertVerified, ServerCertVerifier};
use tide::convert::{Deserialize, Serialize};
use surf::http::{Method, Mime};
use surf::{Request, Url};
use surf::http::headers::{CONTENT_TYPE, HeaderName, HeaderValues, ToHeaderValues};
use tide::{StatusCode};
use crate::errors::{Error, ErrorCode, Result};
pub use json::*;
pub use surf::*;

pub struct NoCertificateVerification {}

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(&self,
                          _roots: &RootCertStore,
                          _presented_certs: &[Certificate],
                          _dns_name: webpki::DNSNameRef,
                          _ocsp_response: &[u8]) -> std::result::Result<ServerCertVerified, rustls::TLSError> {
        Ok(ServerCertVerified::assertion())
    }
}

fn make_config() -> Arc<rustls::ClientConfig> {
    let mut config = rustls::ClientConfig::new();
    config.dangerous()
        .set_certificate_verifier(Arc::new(NoCertificateVerification {}));

    Arc::new(config)
}

fn create_http_client(max_connections: Option<usize>, skip_tls: bool) -> http_client::h1::H1Client {
    use http_client::HttpClient;
    let mut config = http_client::Config::new()
        .set_timeout(Some(Duration::from_secs(30)))
        .set_max_connections_per_host(max_connections.unwrap_or(50))
        .set_http_keep_alive(true);
    if skip_tls {
        config = config.set_tls_config(Some(make_config()));
    }
    let mut client = http_client::h1::H1Client::new();
    client.set_config(config);
    client
}

pub async fn http_post_request(url: &str, param: Vec<u8>, content_type: Option<&str>) -> Result<(Vec<u8>, Option<String>)> {
    let url_obj = Url::parse(url).unwrap();
    let mut req = Request::new(Method::Post, url_obj);
    if content_type.is_some() {
        req.set_content_type(Mime::from(content_type.unwrap()));
    }
    req.set_body(param);
    let mut resp = surf::Client::with_http_client(create_http_client(None, false)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let data = resp.body_bytes().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })?;
    Ok((data, resp.header(CONTENT_TYPE).map(|v| v.last().to_string())))
}

pub async fn http_post_request2<T: for<'de> Deserialize<'de>>(url: &str, param: Vec<u8>, content_type: Option<&str>) -> Result<T> {
    let url_obj = Url::parse(url).unwrap();
    let mut req = Request::new(Method::Post, url_obj);
    if content_type.is_some() {
        req.set_content_type(Mime::from(content_type.unwrap()));
    }
    req.set_body(param);
    let mut resp = surf::Client::with_http_client(create_http_client(None, false)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let tx = resp.body_string().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;
    serde_json::from_str(tx.as_str()).map_err(|err| {
        let msg = format!("recv {} error! err={}", tx, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })
}

pub async fn http_post_request3<T: for<'de> Deserialize<'de>, P: Serialize>(url: &str, param: &P) -> Result<T> {
    let url_obj = Url::parse(url).unwrap();
    let mut req = Request::new(Method::Post, url_obj);
    req.set_content_type(Mime::from("application/json"));
    let param = serde_json::to_string(param).map_err(|e| {
        let msg = format!("serde json encode err {}", e);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::Failed, msg)
    })?;
    req.set_body(param);
    let mut resp = surf::Client::with_http_client(create_http_client(None, false)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let tx = resp.body_string().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;
    serde_json::from_str(tx.as_str()).map_err(|err| {
        let msg = format!("recv {} error! err={}", tx, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })
}

pub async fn http_get_request2<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T> {
    let req = surf::get(url);
    let mut resp = surf::Client::with_http_client(create_http_client(None, false)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! url={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let tx = resp.body_string().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;
    serde_json::from_str(tx.as_str()).map_err(|err| {
        let msg = format!("recv {} error! err={}", tx, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })
}


pub async fn http_get_request(url: &str) -> Result<(Vec<u8>, Option<String>)> {
    let req = surf::get(url);
    let mut resp = surf::Client::with_http_client(create_http_client(None, false)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! url={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let tx = resp.body_bytes().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })?;
    Ok((tx, resp.header(CONTENT_TYPE).map(|v| v.last().to_string())))
}

pub async fn http_get_request3(url: &str) -> Result<Response> {
    let req = surf::get(url);
    let mut resp = surf::Client::with_http_client(create_http_client(None, false)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! url={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;
    Ok(resp)
}

pub async fn http_request(req: http_types::Request) -> Result<Response> {
    let url = req.url().to_string();
    let req = surf::Request::from(req);
    let mut resp = surf::Client::with_http_client(create_http_client(None, false)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! url={} err={}", url, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;
    Ok(resp)
}

pub async fn http_post_json(url: &str, param: JsonValue) -> Result<JsonValue> {
    let url_obj = Url::parse(url).unwrap();
    let host = url_obj.host().unwrap().to_string();

    let mut req = Request::new(Method::Post, url_obj);
    req.set_content_type(Mime::from("application/json"));
    req.set_body(param.to_string());
    let mut resp = surf::Client::with_http_client(create_http_client(None, false)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", host, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let resp_str = resp.body_string().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    json::parse(resp_str.as_str()).map_err(|err| {
        let msg = format!("parse {} error! err={}", resp_str.as_str(), err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })
}


pub async fn http_post_json2<T: for<'de> Deserialize<'de>>(url: &str, param: JsonValue) -> Result<T> {
    let url_obj = Url::parse(url).unwrap();
    let host = url_obj.host().unwrap().to_string();
    let mut req = Request::new(Method::Post, url_obj);
    req.set_content_type(Mime::from("application/json"));
    req.set_body(param.to_string());
    let mut resp = surf::Client::with_http_client(create_http_client(None, false)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", host, err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;

    let tx = resp.body_string().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::ConnectFailed, msg)
    })?;
    serde_json::from_str(tx.as_str()).map_err(|err| {
        let msg = format!("parse {} error! err={}", tx.as_str(), err);
        log::error!("{}", msg.as_str());
        Error::new(ErrorCode::InvalidData, msg)
    })
    // resp.body_json().await.map_err(|err| {
    //     let msg = app_msg!("recv {} error! err={}", tx, err);
    //     log::error!("{}", msg.as_str());
    //     Error::new(ErrorCode::ConnectFailed, msg)
    // })
}

#[derive(Clone)]
pub struct HttpClient {
    client: surf::Client,
}

impl Debug for HttpClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "HttpClient")
    }
}

impl HttpClient {
    pub fn new(max_connections: usize, base_url: Option<&str>) -> Result<Self> {
        let mut config = surf::Config::new()
            .set_http_keep_alive(true)
            .set_max_connections_per_host(max_connections)
            .set_timeout(Some(Duration::from_secs(30)))
            .set_http_client(create_http_client(Some(max_connections), false));
        if base_url.is_some() {
            let base_url = base_url.unwrap();
            let base_url = if base_url.ends_with("/") {
                base_url.to_string()
            } else {
                format!("{}/", base_url)
            };
            let url = Url::parse(base_url.as_str()).map_err(|e| {
                Error::new(ErrorCode::InvalidParam, format!("parse {} failed {}", base_url, e))
            })?;
            config = config.set_base_url(url);
        }
        Ok(Self {
            client: config.try_into().unwrap(),
        })
    }

    pub fn new_with_no_cert_verify(max_connections: usize, base_url: Option<&str>) -> Result<Self> {
        let mut config = surf::Config::new()
            .set_http_keep_alive(true)
            .set_max_connections_per_host(max_connections)
            .set_timeout(Some(Duration::from_secs(30)))
            .set_http_client(create_http_client(Some(max_connections), true));
        if base_url.is_some() {
            let base_url = base_url.unwrap();
            let base_url = if base_url.ends_with("/") {
                base_url.to_string()
            } else {
                format!("{}/", base_url)
            };
            let url = Url::parse(base_url.as_str()).map_err(|e| {
                Error::new(ErrorCode::InvalidParam, format!("parse {} failed {}", base_url, e))
            })?;
            config = config.set_base_url(url);
        }
        Ok(Self {
            client: config.try_into().unwrap(),
        })
    }

    pub async fn get_json<T: for<'de> Deserialize<'de>>(&self, uri: &str) -> Result<T> {
        let mut resp = self.client.get(uri).await.map_err(|err| {
            let msg = format!("http connect error! url={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::ConnectFailed, msg)
        })?;

        let tx = resp.body_string().await.map_err(|err| {
            let msg = format!("http connect error! host={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::ConnectFailed, msg)
        })?;
        serde_json::from_str(tx.as_str()).map_err(|err| {
            let msg = format!("recv {} error! err={}", tx, err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::InvalidData, msg)
        })
    }

    pub async fn get(&self, uri: &str) -> Result<(Vec<u8>, Option<String>)> {
        let mut resp = self.client.get(uri).await.map_err(|err| {
            let msg = format!("http connect error! url={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::ConnectFailed, msg)
        })?;

        let tx = resp.body_bytes().await.map_err(|err| {
            let msg = format!("recv body error! err={}", err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::InvalidData, msg)
        })?;
        Ok((tx, resp.header(CONTENT_TYPE).map(|v| v.last().to_string())))
    }

    pub async fn post_json<T: for<'de> Deserialize<'de>, P: Serialize>(&self, uri: &str, param: &P) -> Result<T> {
        let mut req = self.client.post(uri);

        req = req.content_type(Mime::from("application/json"));
        let param = serde_json::to_string(param).map_err(|e| {
            let msg = format!("serde json encode err {}", e);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::Failed, msg)
        })?;
        req = req.body(param);
        let mut resp = req.await.map_err(|err| {
            let msg = format!("http connect error! host={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::ConnectFailed, msg)
        })?;

        let tx = resp.body_string().await.map_err(|err| {
            let msg = format!("http connect error! host={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::ConnectFailed, msg)
        })?;
        serde_json::from_str(tx.as_str()).map_err(|err| {
            let msg = format!("recv {} error! err={}", tx, err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::InvalidData, msg)
        })
    }

    pub async fn post(&self, uri: &str, param: Vec<u8>, content_type: Option<&str>) -> Result<(Vec<u8>, Option<String>)> {
        let mut req = self.client.post(uri);
        if content_type.is_some() {
            req = req.content_type(Mime::from(content_type.unwrap()));
        }
        req = req.body(param);
        let mut resp = req.await.map_err(|err| {
            let msg = format!("http connect error! host={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::ConnectFailed, msg)
        })?;

        let data = resp.body_bytes().await.map_err(|err| {
            let msg = format!("recv body error! err={}", err);
            log::error!("{}", msg.as_str());
            Error::new(ErrorCode::InvalidData, msg)
        })?;
        Ok((data, resp.header(CONTENT_TYPE).map(|v| v.last().to_string())))
    }
}

pub struct HttpClientBuilder {
    base_url: Option<Url>,
    headers: HashMap<HeaderName, HeaderValues>,
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
    pub fn set_base_url(mut self, base_url: &str) -> Result<Self> {
        let base_url = if base_url.ends_with("/") {
            base_url.to_string()
        } else {
            format!("{}/", base_url)
        };
        let url = Url::parse(base_url.as_str()).map_err(|e| {
            Error::new(ErrorCode::InvalidParam, format!("parse {} failed {}", base_url, e))
        })?;
        Ok(self)
    }
    pub fn add_header(
        mut self,
        name: impl Into<HeaderName>,
        values: impl ToHeaderValues,
    ) -> Result<Self> {
        self.headers
            .insert(name.into(), values.to_header_values().map_err(|e| {
                Error::new(ErrorCode::InvalidParam, format!("convert header value err {}", e))
            })?.collect());
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
        let mut config = http_client::Config::new()
            .set_timeout(self.timeout.clone())
            .set_max_connections_per_host(self.max_connections_per_host.clone())
            .set_http_keep_alive(self.http_keep_alive);
        if !self.verify_tls {
            config = config.set_tls_config(Some(make_config()));
        }
        let mut client = http_client::h1::H1Client::new();
        {
            use http_client::HttpClient;
            client.set_config(config);
        }

        let mut config = surf::Config::new()
            .set_http_keep_alive(self.http_keep_alive)
            .set_max_connections_per_host(self.max_connections_per_host)
            .set_timeout(self.timeout)
            .set_http_client(client);
        config.headers = self.headers;
        if self.base_url.is_some() {
            config = config.set_base_url(self.base_url.unwrap());
        }
        HttpClient {
            client: config.try_into().unwrap(),
        }
    }
}
