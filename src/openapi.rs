#[macro_export]
macro_rules! def_openapi {
    ([$name: ident]
    $($tt:tt)*) => {
        $($tt)*
        fn $name() {}
    };
}
pub use utoipa::*;
pub use paste::paste;

#[macro_export]
macro_rules! add_openapi_item {
    ($api_doc: expr, $name: ident) => {
        sfo_http::openapi::paste! {
            sfo_http::openapi::OpenApiServer::add_api_item::<[<__path_ $name>]>($api_doc);
        }
    };
}

#[macro_export]
macro_rules! add_openapi_schema {
    ($api_doc: expr, $name: ident) => {
        sfo_http::openapi::paste! {
            sfo_http::openapi::OpenApiServer::add_schema_item::<$name>($api_doc);
        }
    };
}

#[cfg(feature = "openapi")]
pub trait OpenApiServer {
    fn set_api_doc(&mut self, api_doc: openapi::OpenApi);
    fn get_api_doc(&mut self) -> &mut openapi::OpenApi;
    fn add_api_item<P: Path>(&mut self) {
        self.get_api_doc().paths.paths.insert(P::path(), P::path_item(Some("")));
    }

    fn add_schema_item<S: for<'a> ToSchema<'a>>(&mut self) {
        if self.get_api_doc().components.is_none() {
            self.get_api_doc().components = Some(openapi::Components::default());
        }
        let (name, obj) = S::schema();
        if self.get_api_doc().components.as_mut().unwrap().schemas.contains_key(&name.to_string()) {
            return;
        }
        self.get_api_doc().components.as_mut().unwrap().schemas.insert(name.to_string(), obj);
    }
}


#[cfg(test)]
mod test_open_api {
    use crate::openapi::ToSchema;

    #[derive(ToSchema)]
    enum Status {
        Active, InActive, Locked,
    }

    def_openapi! {
        [get_status]
        #[utoipa::path(get, path = "/status")]
    }
}
