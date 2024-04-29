use std::future::Future;
use std::sync::Arc;
use actix_web::dev::{fn_factory, Service, ServiceRequest, ServiceResponse};
use actix_web::http::Method;
use futures_util::future::LocalBoxFuture;
use crate::errors::HttpResult;
use super::{Endpoint, EndpointHandler, Response};

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
}
