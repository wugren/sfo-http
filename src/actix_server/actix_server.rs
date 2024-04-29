use std::future::Future;
use std::sync::Arc;
use crate::errors::{ErrorCode, HttpResult, into_http_err};
pub use actix_web::*;
use actix_web::dev::fn_factory;
use actix_web::http::Method;
use crate::actix_server::{Endpoint, EndpointHandler, Request, Response};

pub struct HttpServer<State: Clone + Send + Sync + 'static> {
    server_addr: String,
    port: u16,
    router_list: Vec<(Method, String, EndpointHandler<State>)>,
    state: State,
}

impl<State: 'static + Clone + Send + Sync> HttpServer<State> {
    pub fn new(state: State, server_addr: impl Into<String>, port: u16) -> Self {
        Self {
            server_addr: server_addr.into(),
            port,
            router_list: vec![],
            state,
        }
    }

    pub async fn run(self) -> HttpResult<()> {
        let addr = format!("{}:{}", self.server_addr, self.port);
        ::log::info!("start http server:{}", addr);
        let router_list = self.router_list;
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
}

#[cfg(test)]
mod test_actix {
    use actix_web::http::StatusCode;
    use actix_web::body::BoxBody;
    use serde::{Deserialize, Serialize};
    use crate::actix_server::{HttpServer, Request, Response};

    #[derive(Deserialize, Serialize)]
    pub struct Test {
        a: String,
        b: u16
    }

    #[actix_web::test]
    async fn test() {
        let mut server = HttpServer::new((), "127.0.0.1", 8080);
        server.at("/test/{name}").get(|req: Request<()>| {
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

        println!("listening on 127.0.0.1:8080");
        server.run().await.unwrap();
    }
}
