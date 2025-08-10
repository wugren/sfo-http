use std::io;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use http::StatusCode;
use crate::errors::{http_err, ErrorCode, HttpResult};
use super::{Endpoint, Request, Response};

pub(crate) struct ServeFile {
    path: PathBuf,
}

impl ServeFile {
    /// Create a new instance of `ServeFile`.
    pub(crate) fn init(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = path.as_ref().to_owned().canonicalize()?;
        Ok(Self {
            path: PathBuf::from(file),
        })
    }
}

#[async_trait]
impl<Req: Request, Resp: Response> Endpoint<Req, Resp> for ServeFile {
    async fn call(&self, _: Req) -> HttpResult<Resp> {
        match tokio::fs::File::open(&self.path).await {
            Ok(body) => {
                let mut resp = Resp::new(StatusCode::OK);
                resp.set_body_read(body);
                Ok(resp)
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                log::warn!("File not found: {:?}", &self.path);
                Ok(Resp::new(StatusCode::NOT_FOUND))
            }
            Err(e) => Err(http_err!(ErrorCode::IOError, "read file {:?}", self.path.as_path())),
        }
    }
}
