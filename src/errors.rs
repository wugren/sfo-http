pub(crate) use sfo_result::err as http_err;
pub(crate) use sfo_result::into_err as into_http_err;

#[repr(u16)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ErrorCode {
    Failed,
    InvalidData,
    ConnectFailed,
    InvalidParam,
    ServerError,
    NotFound,
    IOError,
    BadRequest,
}
pub type HttpError = sfo_result::Error<ErrorCode>;
pub type HttpResult<T> = sfo_result::Result<T, ErrorCode>;
