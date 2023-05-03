#![allow(unused)]

use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use std::time::Duration;
use cyfs_base::*;
use http_client::http_types;
use rustls::{Certificate, RootCertStore, ServerCertVerified, ServerCertVerifier};
use tide::convert::{Deserialize, Serialize};
use surf::http::{Method, Mime};
use surf::{Request, Url};
use surf::http::headers::CONTENT_TYPE;
use tide::{Response, StatusCode};

pub struct NoCertificateVerification {}

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(&self,
                          _roots: &RootCertStore,
                          _presented_certs: &[Certificate],
                          _dns_name: webpki::DNSNameRef,
                          _ocsp_response: &[u8]) -> Result<ServerCertVerified, rustls::TLSError> {
        Ok(ServerCertVerified::assertion())
    }
}

fn make_config() -> Arc<rustls::ClientConfig> {
    let mut config = rustls::ClientConfig::new();
    config.dangerous()
        .set_certificate_verifier(Arc::new(NoCertificateVerification {}));

    Arc::new(config)
}

fn create_http_client(max_connections: Option<usize>) -> http_client::h1::H1Client {
    use http_client::HttpClient;
    let mut config = http_client::Config::new()
        .set_timeout(Some(Duration::from_secs(30)))
        .set_max_connections_per_host(max_connections.unwrap_or(50))
        .set_http_keep_alive(true)
        .set_tls_config(Some(make_config()));
    let mut client = http_client::h1::H1Client::new();
    client.set_config(config);
    client
}

pub async fn http_post_request(url: &str, param: Vec<u8>, content_type: Option<&str>) -> BuckyResult<(Vec<u8>, Option<String>)> {
    let url_obj = Url::parse(url).unwrap();
    let mut req = Request::new(Method::Post, url_obj);
    if content_type.is_some() {
        req.set_content_type(Mime::from(content_type.unwrap()));
    }
    req.set_body(param);
    let mut resp = surf::Client::with_http_client(create_http_client(None)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
    })?;

    let data = resp.body_bytes().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::InvalidData, msg)
    })?;
    Ok((data, resp.header(CONTENT_TYPE).map(|v| v.last().to_string())))
}

pub async fn http_post_request2<T: for<'de> Deserialize<'de>>(url: &str, param: Vec<u8>, content_type: Option<&str>) -> BuckyResult<T> {
    let url_obj = Url::parse(url).unwrap();
    let mut req = Request::new(Method::Post, url_obj);
    if content_type.is_some() {
        req.set_content_type(Mime::from(content_type.unwrap()));
    }
    req.set_body(param);
    let mut resp = surf::Client::with_http_client(create_http_client(None)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
    })?;

    let tx = resp.body_string().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
    })?;
    serde_json::from_str(tx.as_str()).map_err(|err| {
        let msg = format!("recv {} error! err={}", tx, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::InvalidData, msg)
    })
}

pub async fn http_post_request3<T: for<'de> Deserialize<'de>, P: Serialize>(url: &str, param: &P) -> BuckyResult<T> {
    let url_obj = Url::parse(url).unwrap();
    let mut req = Request::new(Method::Post, url_obj);
    req.set_content_type(Mime::from("application/json"));
    let param = serde_json::to_string(param).map_err(|e| {
        let msg = format!("serde json encode err {}", e);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::Failed, msg)
    })?;
    req.set_body(param);
    let mut resp = surf::Client::with_http_client(create_http_client(None)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
    })?;

    let tx = resp.body_string().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
    })?;
    serde_json::from_str(tx.as_str()).map_err(|err| {
        let msg = format!("recv {} error! err={}", tx, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::InvalidData, msg)
    })
}

pub async fn http_get_request2<T: for<'de> Deserialize<'de>>(url: &str) -> BuckyResult<T> {
    let req = surf::get(url);
    let mut resp = surf::Client::with_http_client(create_http_client(None)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! url={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
    })?;

    let tx = resp.body_string().await.map_err(|err| {
        let msg = format!("http connect error! host={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
    })?;
    serde_json::from_str(tx.as_str()).map_err(|err| {
        let msg = format!("recv {} error! err={}", tx, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::InvalidData, msg)
    })
}


pub async fn http_get_request(url: &str) -> BuckyResult<(Vec<u8>, Option<String>)> {
    let req = surf::get(url);
    let mut resp = surf::Client::with_http_client(create_http_client(None)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! url={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
    })?;

    let tx = resp.body_bytes().await.map_err(|err| {
        let msg = format!("recv body error! err={}", err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::InvalidData, msg)
    })?;
    Ok((tx, resp.header(CONTENT_TYPE).map(|v| v.last().to_string())))
}

pub async fn http_get_request3(url: &str) -> BuckyResult<surf::Response> {
    let req = surf::get(url);
    let mut resp = surf::Client::with_http_client(create_http_client(None)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! url={}, err={}", url, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
    })?;
    Ok(resp)
}

pub async fn http_request(req: http_types::Request) -> BuckyResult<surf::Response> {
    let url = req.url().to_string();
    let req = surf::Request::from(req);
    let mut resp = surf::Client::with_http_client(create_http_client(None)).send(req).await.map_err(|err| {
        let msg = format!("http connect error! url={} err={}", url, err);
        log::error!("{}", msg.as_str());
        BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
    })?;
    Ok(resp)
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
    pub fn new(max_connections: usize, base_url: Option<&str>) -> BuckyResult<Self> {
        let mut config = surf::Config::new()
            .set_http_keep_alive(true)
            .set_max_connections_per_host(max_connections)
            .set_timeout(Some(Duration::from_secs(30)))
            .set_http_client(create_http_client(Some(max_connections)));
        if base_url.is_some() {
            let base_url = base_url.unwrap();
            let base_url = if base_url.ends_with("/") {
                base_url.to_string()
            } else {
                format!("{}/", base_url)
            };
            let url = Url::parse(base_url.as_str()).map_err(|e| {
                BuckyError::new(BuckyErrorCode::InvalidParam, format!("parse {} failed {}", base_url, e))
            })?;
            config = config.set_base_url(url);
        }
        Ok(Self {
            client: config.try_into().unwrap(),
        })
    }

    pub async fn get_json<T: for<'de> Deserialize<'de>>(&self, uri: &str) -> BuckyResult<T> {
        let mut resp = self.client.get(uri).await.map_err(|err| {
            let msg = format!("http connect error! url={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;

        let tx = resp.body_string().await.map_err(|err| {
            let msg = format!("http connect error! host={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;
        serde_json::from_str(tx.as_str()).map_err(|err| {
            let msg = format!("recv {} error! err={}", tx, err);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })
    }

    pub async fn get(&self, uri: &str) -> BuckyResult<(Vec<u8>, Option<String>)> {
        let mut resp = self.client.get(uri).await.map_err(|err| {
            let msg = format!("http connect error! url={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;

        let tx = resp.body_bytes().await.map_err(|err| {
            let msg = format!("recv body error! err={}", err);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;
        Ok((tx, resp.header(CONTENT_TYPE).map(|v| v.last().to_string())))
    }

    pub async fn post_json<T: for<'de> Deserialize<'de>, P: Serialize>(&self, uri: &str, param: &P) -> BuckyResult<T> {
        let mut req = self.client.post(uri);

        req = req.content_type(Mime::from("application/json"));
        let param = serde_json::to_string(param).map_err(|e| {
            let msg = format!("serde json encode err {}", e);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;
        req = req.body(param);
        let mut resp = req.await.map_err(|err| {
            let msg = format!("http connect error! host={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;

        let tx = resp.body_string().await.map_err(|err| {
            let msg = format!("http connect error! host={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;
        serde_json::from_str(tx.as_str()).map_err(|err| {
            let msg = format!("recv {} error! err={}", tx, err);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })
    }

    pub async fn post(&self, uri: &str, param: Vec<u8>, content_type: Option<&str>) -> BuckyResult<(Vec<u8>, Option<String>)> {
        let mut req = self.client.post(uri);
        if content_type.is_some() {
            req = req.content_type(Mime::from(content_type.unwrap()));
        }
        req = req.body(param);
        let mut resp = req.await.map_err(|err| {
            let msg = format!("http connect error! host={}, err={}", uri, err);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;

        let data = resp.body_bytes().await.map_err(|err| {
            let msg = format!("recv body error! err={}", err);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;
        Ok((data, resp.header(CONTENT_TYPE).map(|v| v.last().to_string())))
    }
}

#[derive(Serialize, Deserialize)]
pub struct HttpJsonResult<T>
{
    pub err: u16,
    pub msg: String,
    pub result: Option<T>
}

impl <T> HttpJsonResult<T>
    where T: Serialize
{
    pub fn from(ret: BuckyResult<T>) -> Self {
        match ret {
            Ok(data) => {
                HttpJsonResult {
                    err: 0,
                    msg: "".to_string(),
                    result: Some(data)
                }
            },
            Err(err) => {
                HttpJsonResult {
                    err: err.code().into(),
                    msg: format!("{}", err),
                    result: None
                }
            }
        }
    }

    pub fn to_response(&self) -> Response {
        let mut resp = Response::new(StatusCode::Ok);
        resp.set_content_type("application/json");
        resp.set_body(serde_json::to_string(self).unwrap());
        resp
    }
}
