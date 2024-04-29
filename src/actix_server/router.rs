use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use actix_web::dev::{fn_factory, Service, ServiceRequest, ServiceResponse};
use actix_web::http::Method;
use futures_util::future::LocalBoxFuture;
use crate::errors::{HttpResult, into_http_err};
use super::{Endpoint, EndpointHandler, Response, ServeDir, ServeFile};

pub struct Route<'a, State: 'static + Clone + Send + Sync> {
    path: String,
    state: State,
    route_list: &'a mut Vec<(Method, String, EndpointHandler<State>)>,
}

impl<'a, State> Route<'a, State>
    where
        State: 'static + Clone + Send + Sync, {
    pub fn new(path: String,
               state: State,
               route_list: &'a mut Vec<(Method, String, EndpointHandler<State>)>,) -> Route<State> {
        Route {
            path,
            state,
            route_list,
        }
    }

    pub fn get(&mut self, ep: impl Endpoint<State>) -> &mut Self {
        self.route_list.push((Method::GET, self.path.clone(), EndpointHandler::new(self.state.clone(), ep)));
        self
    }

    pub fn post(&mut self, ep: impl Endpoint<State>) -> &mut Self {
        self.route_list.push((Method::POST, self.path.clone(), EndpointHandler::new(self.state.clone(), ep)));
        self
    }

    pub fn put(&mut self, ep: impl Endpoint<State>) -> &mut Self {
        self.route_list.push((Method::PUT, self.path.clone(), EndpointHandler::new(self.state.clone(), ep)));
        self
    }

    pub fn delete(&mut self, ep: impl Endpoint<State>) -> &mut Self {
        self.route_list.push((Method::DELETE, self.path.clone(), EndpointHandler::new(self.state.clone(), ep)));
        self
    }

    pub fn serve_dir(&mut self, dir: impl AsRef<Path>) -> HttpResult<&mut Self> {
        let dir = dir.as_ref().to_path_buf().canonicalize()
            .map_err(into_http_err!(crate::errors::ErrorCode::IOError, "serve_dir failed"))?;
        let prefix = self.path.clone();
        self.route_list.push((Method::GET, format!("{}/{{tail:.*}}", prefix.clone()), EndpointHandler::new(self.state.clone(), ServeDir::new(prefix, dir))));
        Ok(self)
    }

    pub fn serve_file(&mut self, file: impl AsRef<Path>) -> HttpResult<&mut Self> {
        self.route_list.push((Method::GET, self.path.clone(), EndpointHandler::new(self.state.clone(), ServeFile::init(file.as_ref().to_path_buf())?)));
        Ok(self)
    }
}
