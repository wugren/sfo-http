#[macro_export]
macro_rules! def_open_api {
    ([$name: ident]
    $($tt:tt)*) => {
        $($tt)*
        fn $name() {}
    };
}
pub use utoipa::*;

#[cfg(test)]
mod test_open_api {
    use crate::openapi::ToSchema;

    #[derive(ToSchema)]
    enum Status {
        Active, InActive, Locked,
    }

    def_open_api! {
        [get_status]
        #[utoipa::path(get, path = "/status")]
    }
}
