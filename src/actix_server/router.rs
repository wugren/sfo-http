use std::fmt::Debug;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use actix_web::dev::{fn_factory, Service, ServiceRequest, ServiceResponse};
use actix_web::http::Method;
use futures_util::future::LocalBoxFuture;
use serde::Serialize;
use crate::errors::{HttpResult, into_http_err};
use crate::http_server::{Endpoint, Route};
use super::{EndpointHandler, ActixResponse, ServeDir, ServeFile, ActixRequest};

pub struct ActixRoute<'a> {
    path: String,
    route_list: &'a mut Vec<(Method, String, EndpointHandler)>,
}

impl<'a> ActixRoute<'a> {
    pub fn new(path: String,
               route_list: &'a mut Vec<(Method, String, EndpointHandler)>,) -> ActixRoute<'a> {
        ActixRoute {
            path,
            route_list,
        }
    }

}

impl<'a> Route<ActixRequest, ActixResponse> for ActixRoute<'a> {
    fn get(&mut self, ep: impl Endpoint<ActixRequest, ActixResponse>) -> &mut Self {
        self.route_list.push((Method::GET, self.path.clone(), EndpointHandler::new(ep)));
        self
    }

    fn post(&mut self, ep: impl Endpoint<ActixRequest, ActixResponse>) -> &mut Self {
        self.route_list.push((Method::POST, self.path.clone(), EndpointHandler::new(ep)));
        self
    }

    fn put(&mut self, ep: impl Endpoint<ActixRequest, ActixResponse>) -> &mut Self {
        self.route_list.push((Method::PUT, self.path.clone(), EndpointHandler::new(ep)));
        self
    }

    fn delete(&mut self, ep: impl Endpoint<ActixRequest, ActixResponse>) -> &mut Self {
        self.route_list.push((Method::DELETE, self.path.clone(), EndpointHandler::new(ep)));
        self
    }

    fn serve_dir(&mut self, dir: impl AsRef<Path>) -> HttpResult<&mut Self> {
        let dir = dir.as_ref().to_path_buf().canonicalize()
            .map_err(into_http_err!(crate::errors::ErrorCode::IOError, "serve_dir failed"))?;
        let prefix = self.path.clone();
        self.route_list.push((Method::GET, format!("{}/{{tail:.*}}", prefix.clone()), EndpointHandler::new(ServeDir::new(prefix, dir))));
        Ok(self)
    }

    fn serve_file(&mut self, file: impl AsRef<Path>) -> HttpResult<&mut Self> {
        self.route_list.push((Method::GET, self.path.clone(), EndpointHandler::new(ServeFile::init(file.as_ref().to_path_buf())?)));
        Ok(self)
    }
}
