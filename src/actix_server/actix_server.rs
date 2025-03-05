use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;
use crate::errors::{ErrorCode, HttpResult, into_http_err};
use actix_web::dev::{fn_factory, ServiceFactory, ServiceRequest};
use actix_web::http::{Method, StatusCode};
use actix_web::{web, App, Error, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
#[cfg(feature = "openapi")]
use utoipa::openapi::OpenApi;
use crate::actix_server::{EndpointHandler, ActixRequest, ActixResponse};
use crate::http_server::Response;
#[cfg(feature = "openapi")]
use crate::openapi::OpenApiServer;

pub struct ActixHttpServer<
    State: Clone + Send + Sync + 'static> {
    server_addr: String,
    port: u16,
    router_list: Vec<(Method, String, EndpointHandler<State>)>,
    state: State,
    #[cfg(feature = "openapi")]
    api_doc: Option<utoipa::openapi::OpenApi>,
    enable_api_doc: bool,
}

#[cfg(feature = "openapi")]
impl<
    State: Clone + Send + Sync + 'static> OpenApiServer for ActixHttpServer<State> {
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

impl<
    State: 'static + Clone + Send + Sync> ActixHttpServer<State> {
    pub fn new(state: State, server_addr: impl Into<String>, port: u16) -> Self {
        Self {
            server_addr: server_addr.into(),
            port,
            router_list: vec![],
            state,
            #[cfg(feature = "openapi")]
            api_doc: None,
            enable_api_doc: false,
        }
    }

    pub async fn run(self) -> HttpResult<()> {
        let addr = format!("{}:{}", self.server_addr, self.port);
        ::log::info!("start http server:{}", addr);
        let router_list = self.router_list;
        #[cfg(feature = "openapi")]
        let api_doc = self.api_doc.clone();

        actix_web::HttpServer::new(move || {
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
        }).bind((self.server_addr.as_str(), self.port))
            .map_err(into_http_err!(ErrorCode::ServerError, "failed to bind server"))?
            .run().await
            .map_err(into_http_err!(ErrorCode::ServerError, "failed to run server"))?;
        Ok(())
    }

    pub fn at(self: &mut Self, path: &str) -> super::router::ActixRoute<State> {
        super::router::ActixRoute::new(path.to_string(), self.state.clone(), &mut self.router_list)
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
        {
            if self.api_doc.is_some() {
                app = app.service(utoipa_swagger_ui::SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", self.api_doc.clone().unwrap()));
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
    use crate::http_server::{Request, Response, Route};
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
        let mut server = ActixHttpServer::<>::new((), "127.0.0.1", 8080);

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
        server.at("/test1/{name}").get(|req: ActixRequest<()>| {
            async move {
                let name = req.param("name").unwrap();
                println!("{}", name);

                let mut resp = ActixResponse::new(StatusCode::OK);
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
        server.at("/test2").post(|mut req: ActixRequest<()>| {
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

        server.at("/test3").serve_dir(".").unwrap();
        println!("listening on 127.0.0.1:8080");


        // server.run().await.unwrap();
    }
}
