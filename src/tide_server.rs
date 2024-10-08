use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tide::http::headers::{COOKIE, HeaderValue};
use tide::security::{CorsMiddleware, Origin};
pub use tide::*;
use tide::http::Mime;
#[cfg(feature = "openapi")]
use utoipa::openapi::{OpenApi, PathItem};
use crate::errors::{ErrorCode, http_err, HttpResult, into_http_err};
#[cfg(feature = "openapi")]
use crate::openapi::OpenApiServer;

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
    pub fn from<C: Debug + Copy + Sync + Send + 'static + Into<u16>>(ret: sfo_result::Result<T, C>) -> Self {
        match ret {
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
        }
    }

    pub fn to_response(&self) -> Response {
        let mut resp = Response::new(StatusCode::Ok);
        resp.set_content_type("application/json");
        resp.set_body(serde_json::to_string(self).unwrap());
        resp
    }
}

pub struct HttpServer<T> {
    app: Server<T>,
    server_addr: String,
    port: u16,
    #[cfg(feature = "openapi")]
    api_doc: Option<OpenApi>,
    enable_api_doc: bool,
}

#[cfg(feature = "openapi")]
impl<T: Clone + Send + Sync + 'static> OpenApiServer for HttpServer<T> {
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

impl<T: Clone + Send + Sync + 'static> HttpServer<T> {
    pub fn new(state: T, server_addr: String, port: u16, allow_origin: Option<Vec<String>>) -> Self {
        let mut app = tide::with_state(state);

        let cors = CorsMiddleware::new()
            .allow_methods(
                "GET, POST, PUT, DELETE, OPTIONS"
                    .parse::<HeaderValue>()
                    .unwrap(),
            )
            .allow_origin(Origin::from(allow_origin.unwrap_or(vec!["*".to_string()])))
            .allow_credentials(true)
            .allow_headers("*".parse::<HeaderValue>().unwrap())
            .expose_headers("*".parse::<HeaderValue>().unwrap());
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
                        Ok(Response::builder(200)
                            .body(json!(api_doc.unwrap()))
                            .build())
                    }
                });
                async fn serve_swagger<T>(request: Request<T>) -> Result<Response> {
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
                                Ok(Response::builder(200)
                                    .body(file.bytes.to_vec())
                                    .content_type(file.content_type.parse::<Mime>().map_err(|e| {
                                        http_err!(ErrorCode::ServerError, "parse mime error {}", e)
                                    })?)
                                    .build())
                            })
                            .unwrap_or_else(|| Ok(Response::builder(404).build())),
                        Err(error) => Ok(Response::builder(500).body(error.to_string()).build()),
                    }
                }

                self.app.at("/doc/*").get(serve_swagger);
                self.app.at("/doc").get(|_| async {
                    Ok(Redirect::new("./doc/"))
                });
                self.app.at("/doc/").get(serve_swagger);
            }
        }
        self.app.listen(addr).await.map_err(into_http_err!(ErrorCode::ServerError, "start http server failed"))?;
        Ok(())
    }
}

impl<T> Deref for HttpServer<T> {
    type Target = Server<T>;

    fn deref(&self) -> &Self::Target {
        &self.app
    }
}

impl<T> DerefMut for HttpServer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.app
    }
}

pub fn get_param<'a, STATE>(req: &'a Request<STATE>, name: &str) -> tide::Result<&'a str> {
    req.param(name)
}

pub fn get_cookie<'a, STATE>(req: &'a Request<STATE>, cookie_name: &str) -> Option<String> {
    let cookie = req.header(COOKIE);
    if cookie.is_none() {
        return None;
    }

    //log::info!("cookie {}", cookie.unwrap().last().as_str());
    let cookie_list: Vec<_> = cookie.unwrap().last().as_str().split(";").collect();
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
