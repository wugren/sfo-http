use std::fmt::Debug;
use std::future::Future;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use actix_cors::Cors;
use crate::errors::{ErrorCode, HttpResult, into_http_err};
use actix_web::dev::{fn_factory, ServiceFactory, ServiceRequest};
use actix_web::http::{Method, StatusCode};
use actix_web::{web, App, Error, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
#[cfg(feature = "openapi")]
use utoipa::openapi::OpenApi;
use crate::actix_server::{EndpointHandler, ActixRequest, ActixResponse, ServeDir, ServeFile};
use crate::http_server::{Endpoint, HttpMethod, HttpServer, HttpServerConfig, Response};
#[cfg(feature = "openapi")]
use crate::openapi::OpenApiServer;

pub struct ActixHttpServer {
    config: HttpServerConfig,
    router_list: Vec<(Method, String, EndpointHandler)>,
    #[cfg(feature = "openapi")]
    api_doc: Option<utoipa::openapi::OpenApi>,
    enable_api_doc: bool,
}

#[cfg(feature = "openapi")]
impl OpenApiServer for ActixHttpServer {
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

impl ActixHttpServer {
    pub fn new(config: HttpServerConfig) -> Self {
        Self {
            config,
            router_list: vec![],
            #[cfg(feature = "openapi")]
            api_doc: None,
            enable_api_doc: false,
        }
    }

    pub async fn run(mut self) -> HttpResult<()> {
        let server_addr = self.config.server_addr.clone();
        let port = self.config.port;
        let addr = format!("{}:{}", self.config.server_addr, self.config.port);
        ::log::info!("start http server:{}", addr);
        let router_list = self.router_list;
        #[cfg(feature = "openapi")]
        let api_doc = self.api_doc.clone();
        let config = self.config.clone();
        actix_web::HttpServer::new(move || {
            let mut cors = Cors::default().allow_any_method();
            if !config.allow_origins.is_empty() {
                for origin in config.allow_origins.iter() {
                    if origin == "*" {
                        cors = cors.send_wildcard().allow_any_origin();
                        break;
                    } else {
                        cors = cors.allowed_origin(origin);
                    }
                }
            }
            if !config.allow_methods.is_empty() {
                if config.allow_methods.contains(&"*".to_string()) {
                    cors = cors.allow_any_method();
                } else {
                    cors = cors.allowed_methods(config.allow_methods.iter().map(|v| Method::from_str(v).unwrap()).collect::<Vec<Method>>());
                }
            }
            if !config.allow_headers.is_empty() {
                if config.allow_headers.contains(&"*".to_string()) {
                    cors = cors.allow_any_header().send_wildcard();
                } else {
                    cors = cors.allowed_headers(config.allow_headers.clone());
                }
            }

            if !config.expose_headers.is_empty() {
                if config.expose_headers.contains(&"*".to_string()) {
                    cors = cors.expose_any_header();
                } else {
                    cors = cors.expose_headers(config.expose_headers.clone());
                }
            }
            if config.support_credentials {
                cors = cors.supports_credentials();
            }
            cors = cors.max_age(Some(config.max_age as usize));

            let mut app = actix_web::App::new().wrap(cors);
            for (method, path, handler) in router_list.iter() {
                let handler = handler.clone();
                if method == &Method::PUT {
                    app = app.route(path.as_str(), web::put().service(fn_factory(move || {
                        let handler = handler.clone();
                        async move {
                            Ok(handler)
                        }
                    })))
                } else if method == &Method::GET {
                    app = app.route(path.as_str(), web::get().service(fn_factory(move || {
                        let handler = handler.clone();
                        async move {
                            Ok(handler)
                        }
                    })))
                } else if method == &Method::POST {
                    app = app.route(path.as_str(), web::post().service(fn_factory(move || {
                        let handler = handler.clone();
                        async move {
                            Ok(handler)
                        }
                    })))
                } else if method == &Method::DELETE {
                    app = app.route(path.as_str(), web::delete().service(fn_factory(move || {
                        let handler = handler.clone();
                        async move {
                            Ok(handler)
                        }
                    })))
                }
            }
            #[cfg(feature = "openapi")]
            {
                let api_doc = api_doc.clone();
                if self.enable_api_doc && api_doc.is_some() {
                    app = app.service(utoipa_swagger_ui::SwaggerUi::new("/doc/{_:.*}").url("/api-docs/openapi.json", api_doc.unwrap()));
                    async fn doc() -> impl Responder {
                        HttpResponse::Found()
                            .append_header(("Location", "/doc/"))
                            .finish()
                    }

                    app = app.route("/doc", web::get().to(doc));
                }
            }
            app
        }).bind((server_addr.as_str(), port))
            .map_err(into_http_err!(ErrorCode::ServerError, "failed to bind server"))?
            .run().await
            .map_err(into_http_err!(ErrorCode::ServerError, "failed to run server"))?;
        Ok(())
    }

    pub fn attach_to_actix_app<F>(&self, mut app: App<F>) -> App<F>
        where
            F: ServiceFactory<ServiceRequest, Config = (), Error = Error, InitError = ()> {

        for (method, path, handler) in self.router_list.iter() {
            let handler = handler.clone();
            if method == &Method::PUT {
                app = app.route(path.as_str(), web::put().service(fn_factory(move || {
                    let handler = handler.clone();
                    async move {
                        Ok(handler)
                    }
                })))
            } else if method == &Method::GET {
                app = app.route(path.as_str(), web::get().service(fn_factory(move || {
                    let handler = handler.clone();
                    async move {
                        Ok(handler)
                    }
                })))
            } else if method == &Method::POST {
                app = app.route(path.as_str(), web::post().service(fn_factory(move || {
                    let handler = handler.clone();
                    async move {
                        Ok(handler)
                    }
                })))
            } else if method == &Method::DELETE {
                app = app.route(path.as_str(), web::delete().service(fn_factory(move || {
                    let handler = handler.clone();
                    async move {
                        Ok(handler)
                    }
                })))
            }
        }
        #[cfg(feature = "openapi")]
        {{
            let api_doc = self.api_doc.clone();
            if self.enable_api_doc && api_doc.is_some() {
                app = app.service(utoipa_swagger_ui::SwaggerUi::new("/doc/{_:.*}").url("/api-docs/openapi.json", api_doc.unwrap()));
                async fn doc() -> impl Responder {
                    HttpResponse::Found()
                        .append_header(("Location", "/doc/"))
                        .finish()
                }

                app = app.route("/doc", web::get().to(doc));
            }
        }
        }
        app
    }
}

impl HttpServer<ActixRequest, ActixResponse> for ActixHttpServer {
    fn serve(&mut self, path: &str, method: HttpMethod, ep: impl Endpoint<ActixRequest, ActixResponse>) {
        let method = match method {
            HttpMethod::GET => Method::GET,
            HttpMethod::POST => Method::POST,
            HttpMethod::PUT => Method::PUT,
            HttpMethod::DELETE => Method::DELETE,
            HttpMethod::PATCH => Method::PATCH,
            HttpMethod::OPTIONS => Method::OPTIONS,
            HttpMethod::HEAD => Method::HEAD,
            HttpMethod::TRACE => Method::TRACE,
            HttpMethod::CONNECT => Method::CONNECT,
            _ => panic!("unsupported method"),
        };
        self.router_list.push((method, path.to_string(), EndpointHandler::new(ep)));
    }

    fn serve_dir(&mut self, path: &str, dir: impl AsRef<Path>) -> HttpResult<()> {
        let dir = dir.as_ref().to_path_buf().canonicalize()
            .map_err(into_http_err!(crate::errors::ErrorCode::IOError, "serve_dir failed"))?;
        self.router_list.push((Method::GET, format!("{}/{{tail:.*}}", path), EndpointHandler::new(ServeDir::new(path.to_string(), dir))));
        Ok(())
    }

    fn serve_file(&mut self, path: &str, file: impl AsRef<Path>) -> HttpResult<()> {
        self.router_list.push((Method::GET, path.to_string(), EndpointHandler::new(ServeFile::init(file.as_ref().to_path_buf())?)));
        Ok(())
    }
}

#[cfg(all(test, feature = "client"))]
mod test_actix {
    use actix_web::http::StatusCode;
    use actix_web::body::BoxBody;
    use serde::{Deserialize, Serialize};
    use tokio::runtime::Handle;
    use crate::actix_server::{ActixHttpServer, ActixRequest, ActixResponse};
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
        let handle = std::thread::spawn(|| {
            let mut server = ActixHttpServer::new(HttpServerConfig::new("127.0.0.1", 8080));

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
            server.serve("/test1/{name}", HttpMethod::GET,|req: ActixRequest| {
                async move {
                    let name = req.param("name").unwrap();
                    println!("{}", name);

                    let mut resp = ActixResponse::new(StatusCode::OK);
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
            server.serve("/test2", HttpMethod::POST,|mut req: ActixRequest| {
                async move {
                    let t: Test = req.query().unwrap();
                    let t2: Test = req.body_json().await.unwrap();

                    let mut resp = ActixResponse::new(StatusCode::OK);
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
            println!("listening on 127.0.0.1:8080");
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let server = rt.block_on(async move {
                server.run().await
            });

        });
        let client = HttpClientBuilder::default().set_base_url("http://127.0.0.1:8081").build();
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

        println!("listening on 127.0.0.1:8080 finish");
    }
}
