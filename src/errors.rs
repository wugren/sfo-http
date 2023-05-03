
#[repr(u16)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ErrorCode {
    Failed,
    InvalidData,
    ConnectFailed,
    InvalidParam,
}

#[derive(Debug)]
pub struct Error {
    code: ErrorCode,
    msg: String,
}

impl Error {
    pub fn new(code: impl Into<ErrorCode>, msg: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            msg: msg.into(),
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn msg(&self) -> &str {
        self.msg.as_str()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
