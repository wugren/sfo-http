use std::collections::HashMap;
use http_body_util::{BodyExt, Full, StreamBody};
use std::fmt::Debug;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use futures_util::{StreamExt, TryStreamExt};
use http::{HeaderName, HeaderValue, Method, StatusCode};
use http::request::Parts;
use http_body_util::combinators::{BoxBody, UnsyncBoxBody};
use hyper::body::{Body, Bytes, Frame, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use route_recognizer::Params;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tide::http::Mime;
use tokio::io::AsyncRead;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use utoipa::openapi::OpenApi;
use crate::errors::{http_err, into_http_err, ErrorCode, HttpError, HttpResult};
use crate::http_server::{Endpoint, HttpMethod, HttpServer, HttpServerConfig, Request, Response, Route, Router};
use crate::openapi::OpenApiServer;
use crate::tide_server::TideHttpServer;

pub struct HyperRequest {
    body: Option<Incoming>,
    head: Parts,
    remote_addr: SocketAddr,
    local_addr: SocketAddr,
    route_params: Params,
}

impl HyperRequest {
    pub fn new(req: hyper::Request<Incoming>, remote_addr: SocketAddr, local_addr: SocketAddr, route_params: Params) -> Self {
        let (head, body) = req.into_parts();
        Self {
            body: Some(body),
            head,
            remote_addr,
            local_addr,
            route_params,
        }
    }
}

#[async_trait::async_trait]
impl Request for HyperRequest {
    fn peer_addr(&self) -> Option<String> {
        Some(self.remote_addr.to_string())
    }

    fn local_addr(&self) -> Option<String> {
        Some(self.local_addr.to_string())
    }

    fn remote(&self) -> Option<String> {
        // 1. 检查 Forwarded 头部的 for 键
        if let Some(forwarded) = self.head.headers.get("Forwarded") {
            if let Ok(forwarded_str) = forwarded.to_str() {
                // 解析 Forwarded 头部，查找 for= 的值
                for part in forwarded_str.split(';') {
                    let trimmed = part.trim();
                    if trimmed.starts_with("for=") {
                        return Some(trimmed[4..].to_string());
                    }
                }
            }
        }

        // 2. 检查第一个 X-Forwarded-For 头部
        if let Some(x_forwarded_for) = self.head.headers.get("X-Forwarded-For") {
            if let Ok(x_forwarded_for_str) = x_forwarded_for.to_str() {
                if let Some(first_addr) = x_forwarded_for_str.split(',').next() {
                    return Some(first_addr.trim().to_string());
                }
            }
        }

        // 3. 使用传输层的对端地址
        self.peer_addr()
    }

    fn host(&self) -> Option<String> {
        // 1. 检查 Forwarded 头部的 host 键
        if let Some(forwarded) = self.head.headers.get("Forwarded") {
            if let Ok(forwarded_str) = forwarded.to_str() {
                // 解析 Forwarded 头部，查找 host= 的值
                for part in forwarded_str.split(';') {
                    let trimmed = part.trim();
                    if trimmed.starts_with("host=") {
                        return Some(trimmed[5..].to_string());
                    }
                }
            }
        }

        // 2. 检查第一个 X-Forwarded-Host 头部
        if let Some(x_forwarded_host) = self.head.headers.get("X-Forwarded-Host") {
            if let Ok(x_forwarded_host_str) = x_forwarded_host.to_str() {
                return Some(x_forwarded_host_str.split(',').next()?.trim().to_string());
            }
        }

        // 3. 检查 Host 头部
        if let Some(host) = self.head.headers.get("Host") {
            if let Ok(host_str) = host.to_str() {
                return Some(host_str.to_string());
            }
        }

        // 4. 从 URL 获取域名
        if let Some(authority) = self.head.uri.authority() {
            return Some(authority.to_string());
        }

        None
    }

    fn path(&self) -> &str {
        self.head.uri.path()
    }

    fn method(&self) -> Method {
        self.head.method.clone()
    }

    fn content_type(&self) -> Option<String> {
        self.head.headers.get("Content-Type").and_then(|v| v.to_str().ok().map(|s| s.to_string()))
    }

    fn header(&self, key: impl Into<HeaderName>) -> Option<HeaderValue> {
        self.head.headers.get(key.into()).map(|v| v.to_owned())
    }

    fn header_all(&self, key: impl Into<HeaderName>) -> Vec<HeaderValue> {
        self.head.headers.get_all(key.into()).iter().map(|v| v.to_owned()).collect()
    }

    fn param(&self, key: &str) -> HttpResult<&str> {
        self.route_params.find(key)
            .ok_or_else(|| http_err!(ErrorCode::InvalidParam, "Param \"{}\" not found", key.to_string()))
    }

    fn query<T: DeserializeOwned>(&self) -> HttpResult<T> {
        let query = self.head.uri.query().unwrap_or("");
        serde_qs::from_str(query).map_err(|e| {
            http_err!(ErrorCode::BadRequest, "{}", e)
        })
    }

    async fn body_string(&mut self) -> HttpResult<String> {
        self.body.take().ok_or(http_err!(ErrorCode::InvalidData, "no body"))?.collect().await
            .map(|body| String::from_utf8_lossy(body.to_bytes().as_ref()).to_string())
            .map_err(|e| http_err!(ErrorCode::IOError))
    }

    async fn body_bytes(&mut self) -> HttpResult<Vec<u8>> {
        self.body.take().ok_or(http_err!(ErrorCode::InvalidData, "no body"))?.collect().await
            .map(|body| body.to_bytes().to_vec())
            .map_err(|e| http_err!(ErrorCode::IOError))
    }

    async fn body_json<T: DeserializeOwned>(&mut self) -> HttpResult<T> {
        self.body.take().ok_or(http_err!(ErrorCode::InvalidData, "no body"))?.collect().await
            .map(|body| serde_json::from_slice(body.to_bytes().as_ref()))
            .map_err(|e| http_err!(ErrorCode::IOError))?
            .map_err(into_http_err!(ErrorCode::InvalidData))
    }

    async fn body_form<T: DeserializeOwned>(&mut self) -> HttpResult<T> {
        self.body.take().ok_or(http_err!(ErrorCode::InvalidData, "no body"))?.collect().await
            .map(|body| serde_urlencoded::from_bytes(body.to_bytes().as_ref()))
            .map_err(|e| http_err!(ErrorCode::IOError))?
            .map_err(into_http_err!(ErrorCode::InvalidData))
    }
}

pub struct HyperResponse {
    resp: hyper::Response<UnsyncBoxBody<Bytes, HttpError>>,
}


#[derive(Serialize, Deserialize)]
struct HttpJsonResult<T>
{
    pub err: u16,
    pub msg: String,
    pub result: Option<T>
}

#[async_trait::async_trait]
impl Response for HyperResponse {
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

        let body = serde_json::to_vec(&result).unwrap();
        let mut resp = hyper::Response::builder().status(StatusCode::OK).body(Full::new(Bytes::new()).map_err(|e| http_err!(ErrorCode::IOError)).boxed_unsync()).unwrap();
        resp.headers_mut().insert("Content-Type", HeaderValue::from_static("application/json"));
        Self {
            resp
        }
    }

    fn new(status: StatusCode) -> Self {
        let resp = hyper::Response::builder()
            .status(status)
            .body(Full::new(Bytes::new()).map_err(|e| http_err!(ErrorCode::IOError)).boxed_unsync()).unwrap();
        Self {
            resp
        }
    }

    fn insert_header(&mut self, name: HeaderName, value: HeaderValue) {
        self.resp.headers_mut().insert(name, value);
    }

    fn set_content_type(&mut self, content_type: &str) -> HttpResult<()> {
        self.resp.headers_mut().insert("Content-Type",
                                       HeaderValue::from_str(content_type).map_err(|e| http_err!(ErrorCode::IOError))?);
        Ok(())
    }

    fn set_body(&mut self, body: Vec<u8>) {
        *self.resp.body_mut() = Full::new(Bytes::from(body)).map_err(|e| http_err!(ErrorCode::IOError)).boxed_unsync();
    }

    fn set_body_read<R: AsyncRead + Send + 'static>(&mut self, reader: R) {
        let stream = tokio_util::io::ReaderStream::with_capacity(
            reader,
            512 * 1024,
        );
        *self.resp.body_mut() = BodyExt::map_err(StreamBody::new(stream.map_ok(Frame::data)), |e| http_err!(ErrorCode::IOError)).boxed_unsync();

    }
}


pub struct HyperHttpServer {
    config: HttpServerConfig,
    router: Router<HyperRequest, HyperResponse>,
    #[cfg(feature = "openapi")]
    api_doc: Option<OpenApi>,
    enable_api_doc: bool,
    global_resp_headers: HashMap<HeaderName, HeaderValue>,
}

#[cfg(feature = "openapi")]
impl OpenApiServer for HyperHttpServer {
    fn set_api_doc(&mut self, api_doc: OpenApi) {
        self.api_doc = Some(api_doc);
    }

    fn get_api_doc(&mut self) -> &mut OpenApi {
        if self.api_doc.is_none() {
            self.api_doc = Some(utoipa::openapi::OpenApiBuilder::new().build());
        }

        self.api_doc.as_mut().unwrap()
    }

    fn enable_api_doc(&mut self, enable: bool) {
        self.enable_api_doc = enable;
    }
}

impl HyperHttpServer {
    pub fn new(config: HttpServerConfig) -> Self {
        let mut headers = HashMap::new();
        if !config.allow_methods.is_empty() {
            if config.allow_methods.contains(&"*".to_string()) {
                headers.insert(hyper::header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"));
            } else {
                headers.insert(hyper::header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_str(&config.allow_methods.join(", ")).unwrap());
            }
        }
        if !config.allow_origins.is_empty() {
            if config.allow_origins.contains(&"*".to_string()) {
                headers.insert(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
            } else {
                headers.insert(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_str(&config.allow_origins.join(", ")).unwrap());
            }
        }
        if !config.allow_headers.is_empty() {
            if config.allow_headers.contains(&"*".to_string()) {
                headers.insert(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("*"));
            } else {
                headers.insert(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_str(&config.allow_headers.join(", ")).unwrap());
            }
        }

        if !config.expose_headers.is_empty() {
            if config.expose_headers.contains(&"*".to_string()) {
                headers.insert(hyper::header::ACCESS_CONTROL_EXPOSE_HEADERS, HeaderValue::from_static("*"));
            } else {
                headers.insert(hyper::header::ACCESS_CONTROL_EXPOSE_HEADERS, HeaderValue::from_str(&config.expose_headers.join(", ")).unwrap());
            }
        }
        if config.support_credentials {
            headers.insert(hyper::header::ACCESS_CONTROL_ALLOW_CREDENTIALS, HeaderValue::from_static("true"));
        }
        headers.insert(hyper::header::ACCESS_CONTROL_MAX_AGE, HeaderValue::from_str(config.max_age.to_string().as_str()).unwrap(),);
        Self {
            config,
            router: Router::new(),
            #[cfg(feature = "openapi")]
            api_doc: None,
            enable_api_doc: true,
            global_resp_headers: headers,
        }
    }

    pub async fn run(mut self) -> HttpResult<JoinHandle<()>> {
        let addr = format!("{}:{}", self.config.server_addr, self.config.port);
        let listener = TcpListener::bind(addr.as_str()).await.map_err(into_http_err!(ErrorCode::Failed, "bind {} failed", addr))?;
        log::info!("Listening on http://{}", addr);
        #[cfg(feature = "openapi")]
        {
            if self.enable_api_doc && self.api_doc.is_some() {
                let api_doc = self.api_doc.clone();
                self.serve("/api-docs/openapi.json", Method::GET, move |_| {
                    let api_doc = api_doc.clone();
                    async move {
                        let mut resp = HyperResponse::new(StatusCode::OK);
                        resp.set_body(serde_json::to_vec(&api_doc.unwrap()).unwrap());
                        Ok(resp)
                    }
                });
                async fn serve_swagger(request: HyperRequest) -> HttpResult<HyperResponse> {
                    let path = request.path().to_string();
                    let tail = if path == "/doc" {
                        ""
                    } else {
                        path.strip_prefix("/doc/").unwrap()
                    };
                    let config = Arc::new(utoipa_swagger_ui::Config::from("/api-docs/openapi.json"));

                    match utoipa_swagger_ui::serve(if tail.is_empty() { "index.html" } else { tail }, config) {
                        Ok(swagger_file) => swagger_file
                            .map(|file| {
                                let mut resp = HyperResponse::new(StatusCode::OK);
                                resp.set_body(file.bytes.to_vec());
                                resp.set_content_type(file.content_type.as_str());
                                Ok(resp)
                            })
                            .unwrap_or_else(|| Ok(HyperResponse::new(StatusCode::FORBIDDEN))),
                        Err(error) => {
                            let mut resp = HyperResponse::new(StatusCode::INTERNAL_SERVER_ERROR);
                            resp.set_body(error.to_string().as_bytes().to_vec());
                            Ok(resp)
                        }
                    }
                }

                self.serve("/doc/*", Method::GET, serve_swagger);
                self.serve("/doc", Method::GET, |_| async {
                    let mut resp = HyperResponse::new(StatusCode::FOUND);
                    resp.insert_header(HeaderName::from_static("Location"), HeaderValue::from_static("./doc/"));
                    Ok(resp)
                });
                self.serve("/doc/", Method::GET, serve_swagger);
            }
        }
        let this = Arc::new(self);
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let this = this.clone();
                    tokio::spawn(async move {
                        if let Err(e) = this.serve_connection(stream).await {
                            log::error!("Error serving connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    log::error!("Error accepting connection: {}", e);
                }
            }
        }
    }

    pub async fn serve_connection(self: &Arc<Self>, stream: TcpStream) -> HttpResult<()> {
        let remote_addr = stream.peer_addr().map_err(into_http_err!(ErrorCode::BadRequest))?;
        let local_addr = stream.local_addr().map_err(into_http_err!(ErrorCode::BadRequest))?;
        let io = TokioIo::new(stream);

        let this = self.clone();
        let service = service_fn(move |req| {
            let this = this.clone();
            async move {
                println!("Request: {:?}", req);
                println!("Request: uri {}", req.uri().to_string());
                let selection = this.router.route(req.uri().path(), req.method().clone());
                let req = HyperRequest::new(req, remote_addr, local_addr, selection.params);
                let ret = selection.endpoint.call(req).await;
                match ret {
                    Ok(mut resp) => {
                        for (k, v) in this.global_resp_headers.iter() {
                            resp.insert_header(k.clone(), v.clone());
                        }
                        Ok::<_, hyper::Error>(resp.resp)
                    },
                    Err(err) => {
                        log::error!("Error: {}", err);
                        let resp = HyperResponse::new(StatusCode::INTERNAL_SERVER_ERROR);
                        Ok::<_, hyper::Error>(resp.resp)
                    }
                }
            }
        });

        if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
            log::info!("Failed to serve connection: {:?}", err);
        }
        Ok(())
    }
}

impl HttpServer<HyperRequest, HyperResponse> for HyperHttpServer {
    fn serve(&mut self, path: &str, method: HttpMethod, ep: impl Endpoint<HyperRequest, HyperResponse>) {
        match method {
            HttpMethod::GET => {
                Route::new(&mut self.router, path.to_string()).get(ep);
            },
            HttpMethod::POST => {
                Route::new(&mut self.router, path.to_string()).post(ep);
            },
            HttpMethod::PUT => {
                Route::new(&mut self.router, path.to_string()).put(ep);
            },
            HttpMethod::DELETE => {
                Route::new(&mut self.router, path.to_string()).delete(ep);
            },
            HttpMethod::PATCH => {
                Route::new(&mut self.router, path.to_string()).patch(ep);
            },
            HttpMethod::OPTIONS => {
                Route::new(&mut self.router, path.to_string()).options(ep);
            },
            HttpMethod::HEAD => {
                Route::new(&mut self.router, path.to_string()).head(ep);
            },
            HttpMethod::CONNECT => {
                Route::new(&mut self.router, path.to_string()).connect(ep);
            },
            HttpMethod::TRACE => {
                Route::new(&mut self.router, path.to_string()).trace(ep);
            },
            _ => {
                panic!("unsupported method");
            }
         }
    }

    fn serve_dir(&mut self, path: &str, dir: impl AsRef<Path>) -> HttpResult<()> {
        Route::new(&mut self.router, path.to_string()).serve_dir(dir).map_err(into_http_err!(ErrorCode::Failed, "serve dir failed"))
    }

    fn serve_file(&mut self, path: &str, file: impl AsRef<Path>) -> HttpResult<()> {
        Route::new(&mut self.router, path.to_string()).serve_file(file).map_err(into_http_err!(ErrorCode::Failed, "serve file failed"))
    }
}

#[cfg(all(test, feature = "client"))]
mod test_hyper {
    use http::StatusCode;
    use serde::{Deserialize, Serialize};
    #[cfg(feature = "openapi")]
    use utoipa::ToSchema;
    #[cfg(feature = "openapi")]
    use crate::def_openapi;
    #[cfg(feature = "openapi")]
    use utoipa::OpenApi;
    #[cfg(feature = "openapi")]
    use crate::add_openapi_item;
    #[cfg(feature = "openapi")]
    use crate as sfo_http;
    use crate::http_server::{HttpMethod, HttpServer, HttpServerConfig, Request, Response};
    use crate::http_util::HttpClientBuilder;
    use crate::hyper_server::{HyperHttpServer, HyperRequest, HyperResponse};
    #[cfg(feature = "openapi")]
    use crate::openapi::OpenApiServer;

    #[cfg(feature = "openapi")]
    #[derive(Deserialize, Serialize, ToSchema)]
    pub struct Test {
        a: String,
        b: u16
    }

    #[cfg(not(feature = "openapi"))]
    #[derive(Deserialize, Serialize)]
    pub struct Test {
        a: String,
        b: u16
    }

    #[cfg(feature = "openapi")]
    #[derive(utoipa::OpenApi)]
    #[openapi(paths(), components())]
    struct ApiDoc;

    #[actix_web::test]
    async fn test() {
        let mut server = HyperHttpServer::new(HttpServerConfig::new("127.0.0.1", 8082));

        #[cfg(feature = "openapi")]
        {
            let openapi = ApiDoc::openapi();
            server.set_api_doc(openapi);
        }

        #[cfg(feature = "openapi")]
        def_openapi! {
            [test1]
            #[utoipa::path(
                get,
                path = "/test1/{name}",
                responses(
                    (status = 200, description = "test", body = String)
                ),
                params(
                    ("name" = String, Path, description = "test name"),
                )
            )]
        }
        server.serve("/test1/:name", HttpMethod::GET, |req: HyperRequest| {
            async move {
                let name = req.param("name").unwrap();
                println!("{}", name);

                let mut resp = HyperResponse::new(StatusCode::OK);
                resp.set_content_type("application/text");
                resp.set_body(name.as_bytes().to_owned());
                Ok(resp)
            }
        });
        #[cfg(feature = "openapi")]
        add_openapi_item!(&mut server, test1);

        #[cfg(feature = "openapi")]
        def_openapi! {
            [test2]
            #[utoipa::path(
                post,
                path = "/test2",
                responses(
                    (status = 200, description = "test", body = inline(Test))
                ),
                params(
                    ("a" = String, Query, description = "test a"),
                    ("b" = u16, Query, description = "test b"),
                ),
                request_body = Test,
            )]
        }
        server.serve("/test2", HttpMethod::POST, |mut req: HyperRequest| {
            async move {
                let t: Test = req.query().unwrap();
                let t2: Test = req.body_json().await.unwrap();

                let mut resp = HyperResponse::new(StatusCode::OK);
                resp.set_body(serde_json::to_string(&t).unwrap().as_bytes().to_owned());
                resp.set_body(serde_json::to_string(&t2).unwrap().as_bytes().to_owned());
                Ok(resp)
            }
        });
        {
            let server1 = &mut server;
            #[cfg(feature = "openapi")]
            add_openapi_item!(server1, test2);
        }

        server.serve_dir("/test3", ".").unwrap();
        println!("listening on 127.0.0.1:8082");

        let handle = tokio::spawn(async move {
            server.run().await.unwrap();
        });

        let client = HttpClientBuilder::default().set_base_url("http://127.0.0.1:8082").build();
        let params = Test {
            a: "test".to_string(),
            b: 1,
        };
        let resp = client.post(format!("/test2?{}", serde_urlencoded::to_string(&params).unwrap()).as_str(), serde_json::to_vec(&params).unwrap(), Some("application/json")).await;
        assert!(resp.is_ok());
        let resp = serde_json::from_slice::<Test>(&resp.unwrap().0).unwrap();
        assert_eq!(resp.a, "test");
        assert_eq!(resp.b, 1);

        let resp = client.get("/test1/test").await;
        assert!(resp.is_ok());
        assert_eq!(resp.unwrap().0, "test".as_bytes());

        let resp = client.get("/test3/Cargo.toml").await;
        assert!(resp.is_ok());
        assert_eq!(resp.unwrap().0, include_bytes!("../../Cargo.toml"));

        handle.abort();
        println!("listening on 127.0.0.1:8082 finish");
    }
}
