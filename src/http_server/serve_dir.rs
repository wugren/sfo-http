
use std::path::{Path, PathBuf};
use std::{ffi::OsStr, io};
use http::StatusCode;
use crate::errors::{http_err, ErrorCode, HttpResult};
use super::{Endpoint, Request, Response};

pub(crate) struct ServeDir {
    prefix: String,
    dir: PathBuf,
}

impl ServeDir {
    /// Create a new instance of `ServeDir`.
    pub(crate) fn new(prefix: String, dir: PathBuf) -> Self {
        Self { prefix, dir }
    }
}

#[async_trait::async_trait]
impl<Req: Request, Resp: Response> Endpoint<Req, Resp> for ServeDir
{
    async fn call(&self, req: Req) -> HttpResult<Resp> {
        let path = req.path();
        let path = path.strip_prefix(&self.prefix).unwrap();
        let path = path.trim_start_matches('/');
        let mut file_path = self.dir.clone();
        for p in Path::new(path) {
            if p == OsStr::new(".") {
                continue;
            } else if p == OsStr::new("..") {
                file_path.pop();
            } else {
                file_path.push(&p);
            }
        }

        log::info!("Requested file: {:?}", file_path);

        if !file_path.starts_with(&self.dir) {
            log::warn!("Unauthorized attempt to read: {:?}", file_path);
            Ok(Response::new(StatusCode::FORBIDDEN))
        } else {

            match tokio::fs::File::open(file_path.as_path()).await {
                Ok(body) => {
                    let mut resp = Resp::new(StatusCode::OK);
                    resp.set_body_read(body);
                    Ok(resp)
                },
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    log::warn!("File not found: {:?}", &file_path);
                    Ok(Resp::new(StatusCode::NOT_FOUND))
                }
                Err(e) => Err(http_err!(ErrorCode::IOError, "read file {:?}", file_path.as_path())),
            }
        }
    }
}
