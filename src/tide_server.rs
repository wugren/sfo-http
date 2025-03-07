use std::fmt::Debug;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::pin::Pin;
use std::slice::Iter;
use std::sync::Arc;
use std::task::{Context, Poll};
use http::header::COOKIE;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use serde_json::json;
use serde_json::ser::State;
use tide::security::{CorsMiddleware, Origin};
use tide::http::Mime;
use tide::Server;
#[cfg(feature = "openapi")]
use utoipa::openapi::{OpenApi, PathItem};
use crate::errors::{ErrorCode, http_err, HttpResult, into_http_err};
use crate::http_server::{Endpoint, HttpMethod, HttpServer, Request, Response};
#[cfg(feature = "openapi")]
use crate::openapi::OpenApiServer;

pub struct TideRequest {
    req: tide::Request<()>,
}

impl TideRequest {
    pub fn new(req: tide::Request<()>) -> Self {
        Self {
            req
        }
    }
}

#[async_trait::async_trait(?Send)]
impl crate::http_server::Request for TideRequest {
    fn peer_addr(&self) -> Option<String> {
        self.req.peer_addr().map(ToString::to_string)
    }

    fn local_addr(&self) -> Option<String> {
        self.req.local_addr().map(ToString::to_string)
    }

    fn remote(&self) -> Option<String> {
        self.req.remote().map(ToString::to_string)
    }

    fn host(&self) -> Option<String> {
        self.req.host().map(ToString::to_string)
    }

    fn content_type(&self) -> Option<String> {
        self.req.content_type().map(|v| v.to_string())
    }

    fn header(&self, key: impl Into<http::HeaderName>) -> Option<http::HeaderValue> {
        let header_name = key.into();
        if let Some(values) = self.req.header(tide::http::headers::HeaderName::from(header_name.as_str())) {
            for value in values {
                if let Ok(value) = http::HeaderValue::from_str(value.as_str()) {
                    return Some(value);
                }
            }
        }
        None
    }

    fn header_all(&self, key: impl Into<http::HeaderName>) -> Vec<http::HeaderValue> {
        let mut list = Vec::new();
        let header_name = key.into();
        if let Some(values) = self.req.header(tide::http::headers::HeaderName::from(header_name.as_str())) {
            for value in values {
                if let Ok(value) = http::HeaderValue::from_str(value.as_str()) {
                    list.push(value);
                }
            }
        }
        list
    }

    fn param(&self, key: &str) -> HttpResult<&str> {
        self.req.param(key).map_err(|e| http_err!(ErrorCode::InvalidData, "{}", e))
    }

    fn query<T: DeserializeOwned>(&self) -> HttpResult<T> {
        self.req.query().map_err(|e| http_err!(ErrorCode::InvalidData, "{}", e))
    }

    async fn body_string(&mut self) -> HttpResult<String> {
        self.req.body_string().await.map_err(|e| http_err!(ErrorCode::InvalidData, "{}", e))
    }

    async fn body_bytes(&mut self) -> HttpResult<Vec<u8>> {
        self.req.body_bytes().await.map_err(|e| http_err!(ErrorCode::InvalidData, "{}", e))
    }

    async fn body_json<T: DeserializeOwned>(&mut self) -> HttpResult<T> {
        self.req.body_json().await.map_err(|e| http_err!(ErrorCode::InvalidData, "{}", e))
    }

    async fn body_form<T: DeserializeOwned>(&mut self) -> HttpResult<T> {
        self.req.body_form().await.map_err(|e| http_err!(ErrorCode::InvalidData, "{}", e))
    }
}
unsafe impl Send for TideRequest {}

#[derive(Serialize, Deserialize)]
struct HttpJsonResult<T>
{
    pub err: u16,
    pub msg: String,
    pub result: Option<T>
}

pub struct TideResponse {
    resp: tide::Response,
}

impl crate::http_server::Response for TideResponse {
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
        let mut resp = tide::Response::new(tide::StatusCode::Ok);
        resp.set_content_type("application/json");
        resp.set_body(serde_json::to_string(&result).unwrap());
        Self {
            resp
        }
    }

    fn new(status: http::StatusCode) -> Self {
        let mut resp = tide::Response::new(tide::StatusCode::try_from(status.as_u16()).unwrap());
        Self {
            resp
        }
    }

    fn insert_header(&mut self, name: http::HeaderName, value: http::HeaderValue) {
        self.resp.append_header(tide::http::headers::HeaderName::from(name.as_str()), vec![tide::http::headers::HeaderValue::from_bytes(value.as_bytes().to_vec()).unwrap()].as_slice());
    }

    fn set_content_type(&mut self, content_type: &str) -> HttpResult<()> {
        self.resp.set_content_type(content_type);
        Ok(())
    }

    fn set_body(&mut self, body: Vec<u8>) {
        self.resp.set_body(body);
    }
}

unsafe impl Send for TideResponse {}

struct TideEndpoint {
    ep: Arc<dyn Endpoint<TideRequest, TideResponse>>,
    req: Option<TideRequest>,
    future: Option<Pin<Box<dyn Future<Output = HttpResult<TideResponse>>>>>,
}

impl TideEndpoint {
    pub fn new(ep: Arc<dyn Endpoint<TideRequest, TideResponse>>, req: TideRequest) -> Self {
        Self {
            ep,
            req: Some(req),
            future: None,
        }
    }
}

unsafe impl Send for TideEndpoint {}

impl Future for TideEndpoint {
    type Output = tide::Result<tide::Response>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.future.is_none() {
            let req = self.req.take().unwrap();
            let this: &'static mut Self = unsafe { std::mem::transmute(self.as_mut().get_unchecked_mut()) };
            let future = Box::pin(this.ep.call(req));
            self.as_mut().future = Some(future);
        }
        match Pin::new(self.future.as_mut().unwrap()).poll(cx) {
            Poll::Ready(ret) => {
                match ret {
                    Ok(resp) => {
                        Poll::Ready(Ok(resp.resp))
                    }
                    Err(err) => {
                        Poll::Ready(Err(tide::Error::new(tide::StatusCode::BadRequest, err)))
                    }
                }
            }
            Poll::Pending => {
                Poll::Pending
            }
        }
    }
}

pub struct TideHttpServer {
    app: Server<()>,
    server_addr: String,
    port: u16,
    #[cfg(feature = "openapi")]
    api_doc: Option<OpenApi>,
    enable_api_doc: bool,
}

#[cfg(feature = "openapi")]
impl OpenApiServer for TideHttpServer {
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

impl TideHttpServer {
    pub fn new(server_addr: String, port: u16, allow_origin: Option<Vec<String>>, allow_headers: Option<String>, ) -> Self {
        let mut app = tide::with_state(());

        let mut cors = CorsMiddleware::new()
            .allow_methods(
                "GET, POST, PUT, DELETE, OPTIONS"
                    .parse::<tide::http::headers::HeaderValue>()
                    .unwrap(),
            )
            .allow_origin(Origin::from(allow_origin.unwrap_or(vec!["*".to_string()])))
            .allow_credentials(true);
        if allow_headers.is_some() {
            cors = cors.allow_headers(allow_headers.as_ref().unwrap().as_str().parse::<tide::http::headers::HeaderValue>().unwrap())
                .expose_headers(allow_headers.as_ref().unwrap().as_str().parse::<tide::http::headers::HeaderValue>().unwrap());
        }
        app.with(cors);

        Self {
            app,
            server_addr,
            port,
            #[cfg(feature = "openapi")]
            api_doc: None,
            enable_api_doc: true,
        }
    }

    pub async fn run(mut self) -> HttpResult<()> {
        let addr = format!("{}:{}", self.server_addr, self.port);
        ::log::info!("start http server:{}", addr);
        #[cfg(feature = "openapi")]
        {
            if self.enable_api_doc && self.api_doc.is_some() {
                let api_doc = self.api_doc.clone();
                self.app.at("/api-docs/openapi.json").get(move |_| {
                    let api_doc = api_doc.clone();
                    async move {
                        Ok(tide::Response::builder(200)
                            .body(json!(api_doc.unwrap()))
                            .build())
                    }
                });
                async fn serve_swagger<T>(request: tide::Request<T>) -> tide::Result<tide::Response> {
                    let path = request.url().path().to_string();
                    let tail = if path == "/doc" {
                        ""
                    } else {
                        path.strip_prefix("/doc/").unwrap()
                    };
                    let config = Arc::new(utoipa_swagger_ui::Config::from("/api-docs/openapi.json"));

                    match utoipa_swagger_ui::serve(if tail.is_empty() {"index.html"} else {tail}, config) {
                        Ok(swagger_file) => swagger_file
                            .map(|file| {
                                Ok(tide::Response::builder(200)
                                    .body(file.bytes.to_vec())
                                    .content_type(file.content_type.parse::<Mime>().map_err(|e| {
                                        http_err!(ErrorCode::ServerError, "parse mime error {}", e)
                                    })?)
                                    .build())
                            })
                            .unwrap_or_else(|| Ok(tide::Response::builder(404).build())),
                        Err(error) => Ok(tide::Response::builder(500).body(error.to_string()).build()),
                    }
                }

                self.app.at("/doc/*").get(serve_swagger);
                self.app.at("/doc").get(|_| async {
                    Ok(tide::Redirect::new("./doc/"))
                });
                self.app.at("/doc/").get(serve_swagger);
            }
        }
        self.app.listen(addr).await.map_err(into_http_err!(ErrorCode::ServerError, "start http server failed"))?;
        Ok(())
    }
}

impl HttpServer<TideRequest, TideResponse> for TideHttpServer {
    fn serve(&mut self, path: &str, method: HttpMethod, ep: impl Endpoint<TideRequest, TideResponse>) {
        let ep = Arc::new(ep);
        match method {
            HttpMethod::GET => {
                self.app.at(path).get(move |req| {
                    let ep = ep.clone();
                    let req = TideRequest::new(req);
                    TideEndpoint::new(ep, req)
                });
            }
            HttpMethod::POST => {
                self.app.at(path).post(move |req| {
                    let ep = ep.clone();
                    let req = TideRequest::new(req);
                    TideEndpoint::new(ep, req)
                });
            }
            HttpMethod::PUT => {
                self.app.at(path).put(move |req| {
                    let ep = ep.clone();
                    let req = TideRequest::new(req);
                    TideEndpoint::new(ep, req)
                });
            }
            HttpMethod::DELETE => {
                self.app.at(path).delete(move |req| {
                    let ep = ep.clone();
                    let req = TideRequest::new(req);
                    TideEndpoint::new(ep, req)
                });
            }
        }

    }

    fn serve_dir(&mut self, path: &str, dir: impl AsRef<Path>) -> HttpResult<()> {
        self.app.at(path).serve_dir(dir).map_err(into_http_err!(ErrorCode::ServerError))
    }

    fn serve_file(&mut self, path: &str, file: impl AsRef<Path>) -> HttpResult<()> {
        self.app.at(path).serve_file(file).map_err(into_http_err!(ErrorCode::ServerError))
    }
}

#[cfg(test)]
mod test_tide {
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
    use crate::http_server::{HttpMethod, HttpServer, Request, Response};
    #[cfg(feature = "openapi")]
    use crate::openapi::OpenApiServer;
    use crate::tide_server::{TideHttpServer, TideRequest, TideResponse};

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
        let mut server = TideHttpServer::<>::new("127.0.0.1".to_string(), 8081, None, None);

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
        server.serve("/test1/:name", HttpMethod::GET, |req: TideRequest| {
            async move {
                let name = req.param("name").unwrap();
                println!("{}", name);

                let mut resp = TideResponse::new(StatusCode::OK);
                resp.set_content_type("application/text");
                resp.set_body("test".as_bytes().to_owned());
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
        server.serve("/test2", HttpMethod::POST, |mut req: TideRequest| {
            async move {
                let t: Test = req.query().unwrap();
                let t2: Test = req.body_json().await.unwrap();

                let mut resp = TideResponse::new(StatusCode::OK);
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
        println!("listening on 127.0.0.1:8081");


        server.run().await.unwrap();
        std::future::pending::<()>().await;
        println!("listening on 127.0.0.1:8081 finish");
    }
}
