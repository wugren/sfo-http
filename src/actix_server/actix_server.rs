use std::future::Future;
use std::sync::Arc;
use crate::errors::{ErrorCode, HttpResult, into_http_err};
pub use actix_web::*;
pub use actix_web::HttpServer as ActixHttpServer;
use actix_web::dev::{fn_factory, ServiceFactory, ServiceRequest};
use actix_web::http::Method;
use crate::actix_server::{Endpoint, EndpointHandler, Request, Response};

pub struct HttpServer<State: Clone + Send + Sync + 'static> {
    server_addr: String,
    port: u16,
    router_list: Vec<(Method, String, EndpointHandler<State>)>,
    state: State,
    #[cfg(feature = "openapi")]
    api_doc: Option<utoipa::openapi::OpenApi>
}

impl<State: 'static + Clone + Send + Sync> HttpServer<State> {
    pub fn new(state: State, server_addr: impl Into<String>, port: u16) -> Self {
        Self {
            server_addr: server_addr.into(),
            port,
            router_list: vec![],
            state,
            #[cfg(feature = "openapi")]
            api_doc: None,
        }
    }

    #[cfg(feature = "openapi")]
    pub fn set_api_doc(&mut self, api_doc: crate::openapi::openapi::OpenApi) {
        self.api_doc = Some(api_doc);
    }

    pub async fn run(self) -> HttpResult<()> {
        let addr = format!("{}:{}", self.server_addr, self.port);
        ::log::info!("start http server:{}", addr);
        let router_list = self.router_list;
        #[cfg(feature = "openapi")]
        let api_doc = self.api_doc.clone();

        actix_web::HttpServer::new(move || {
            #[cfg(feature = "openapi")]
            let api_doc = api_doc.clone();
            let mut app = actix_web::App::new();
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
                if api_doc.is_some() {
                    app = app.service(utoipa_swagger_ui::SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", api_doc.unwrap()))
                }
            }
            app
        }).bind((self.server_addr.as_str(), self.port))
            .map_err(into_http_err!(ErrorCode::ServerError, "failed to bind server"))?
            .run().await
            .map_err(into_http_err!(ErrorCode::ServerError, "failed to run server"))?;
        Ok(())
    }

    pub fn at(self: &mut Self, path: &str) -> super::router::Route<State> {
        super::router::Route::new(path.to_string(), self.state.clone(), &mut self.router_list)
    }

    pub fn attach_to_actix_app<T>(&self, mut app: App<T>) -> App<T>
        where
            T: ServiceFactory<ServiceRequest, Config = (), Error = Error, InitError = ()> {

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
        {
            if self.api_doc.is_some() {
                app = app.service(utoipa_swagger_ui::SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", self.api_doc.clone().unwrap()))
            }
        }
        app
    }
}

#[cfg(test)]
mod test_actix {
    use actix_web::http::StatusCode;
    use actix_web::body::BoxBody;
    use serde::{Deserialize, Serialize};
    use crate::actix_server::{HttpServer, Request, Response};
    #[cfg(feature = "openapi")]
    use utoipa::ToSchema;
    #[cfg(feature = "openapi")]
    use crate::def_open_api;
    #[cfg(feature = "openapi")]
    use utoipa::OpenApi;

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
    def_open_api! {
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

    #[cfg(feature = "openapi")]
    def_open_api!{
        [test2]
        #[utoipa::path(
            post,
            path = "/test2",
            responses(
                (status = 200, description = "test", body = Test)
            ),
            params(
                ("a" = String, Query, description = "test a"),
                ("b" = u16, Query, description = "test b"),
            ),
            request_body = Test,
        )]
    }

    #[cfg(feature = "openapi")]
    #[derive(utoipa::OpenApi)]
    #[openapi(paths(test1, test2), components(schemas(Test)))]
    struct ApiDoc;

    #[actix_web::test]
    async fn test() {
        let mut server = HttpServer::new((), "127.0.0.1", 8080);
        server.at("/test1/{name}").get(|req: Request<()>| {
            async move {
                let name = req.param("name").unwrap();
                println!("{}", name);

                let mut resp = Response::new(StatusCode::OK);
                resp.set_body("test");
                Ok(resp)
            }
        });

        server.at("/test2").post(|mut req: Request<()>| {
            async move {
                let t: Test = req.query().unwrap();
                let t2: Test = req.body_json().await.unwrap();

                let mut resp = Response::new(StatusCode::OK);
                resp.set_body(serde_json::to_string(&t).unwrap());
                resp.set_body(serde_json::to_string(&t2).unwrap());
                Ok(resp)
            }
        });

        server.at("/test3").serve_dir(".").unwrap();
        println!("listening on 127.0.0.1:8080");

        #[cfg(feature = "openapi")]
        {
            let openapi = ApiDoc::openapi();
            server.set_api_doc(openapi);
        }

        // server.run().await.unwrap();
    }
}
