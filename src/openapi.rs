#[macro_export]
macro_rules! def_openapi {
    ([$name: ident]
    $($tt:tt)*) => {
        $($tt)*
        fn $name() {}
    };
}
pub use utoipa;
pub use paste::paste;
use utoipa::{openapi, Path, ToSchema};
use utoipa::openapi::path::PathItemBuilder;
use utoipa::openapi::PathItem;

#[macro_export]
macro_rules! add_openapi_item {
    ($api_doc: expr, $name: ident) => {
        sfo_http::openapi::paste! {
            {
                use sfo_http::openapi::utoipa::Path;
                #[allow(non_camel_case_types)]
                struct [<___path_ $name>];
                #[allow(non_camel_case_types)]
                impl sfo_http::openapi::utoipa::__dev::PathConfig for [<___path_ $name>] {
                    fn path() -> String {
                        [<__path_ $name>]::path()
                    }
                    fn methods() -> Vec<sfo_http::openapi::utoipa::openapi::path::HttpMethod> {
                        [<__path_ $name>]::methods()
                    }
                    fn tags_and_operation() -> (Vec<&'static str>, sfo_http::openapi::utoipa::openapi::path::Operation)
                    {
                        let item = [<__path_ $name>]::operation();
                        let mut tags = <[<__path_ $name>] as sfo_http::openapi::utoipa::__dev::Tags>::tags();
                        if !"".is_empty() && tags.is_empty() {
                            tags.push("");
                        }
                        (tags, item)
                    }
                }
                sfo_http::openapi::OpenApiServer::add_api_item::<[<___path_ $name>]>($api_doc);
            }
        }
    };
}

#[macro_export]
macro_rules! add_openapi_schema {
    ($api_doc: expr, $name: ty) => {
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
        let methods = P::methods();
        let operation = P::operation();

        // for one operation method avoid clone
        let path_item = if methods.len() == 1 {
            PathItem::new(
                methods
                    .into_iter()
                    .next()
                    .expect("must have one operation method"),
                operation,
            )
        } else {
            methods
                .into_iter()
                .fold(PathItemBuilder::new(), |path_item, method| {
                    path_item.operation(method, operation.clone())
                })
                .build()
        };
        self.get_api_doc().paths.paths.insert(P::path(), path_item);
    }

    fn add_schema_item<S: ToSchema>(&mut self) {
        if self.get_api_doc().components.is_none() {
            self.get_api_doc().components = Some(openapi::Components::default());
        }
        let name = S::name();
        let obj = S::schema();
        if self.get_api_doc().components.as_mut().unwrap().schemas.contains_key(&name.to_string()) {
            return;
        }
        self.get_api_doc().components.as_mut().unwrap().schemas.insert(name.to_string(), obj);
    }
    fn enable_api_doc(&mut self, enable: bool);
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
