mod actix_server;
mod endpoint;
mod router;

use actix_web::http::header::COOKIE;
pub use actix_server::*;
pub use endpoint::*;
use crate::http_util::header::ToStrError;

pub fn get_cookie<'a, STATE>(req: &'a Request<STATE>, cookie_name: &str) -> Option<String> {
    let cookie = req.header_all(COOKIE).last();
    if cookie.is_none() {
        return None;
    }
    let last_cookie = match cookie.unwrap().to_str() {
        Ok(cookie) => {
            cookie
        }
        Err(_) => {
            return None;
        }
    };
    //log::info!("cookie {}", cookie.unwrap().last().as_str());
    let cookie_list: Vec<_> = last_cookie.split(";").collect();
    let cookie_list: Vec<(String, String)> = cookie_list.into_iter().map(|v| {
        let cookie_list: Vec<_> = v.split("=").collect();
        cookie_list
    }).filter(|v| v.len() == 2).map(|v| (v[0].trim().to_string(), v[1].trim().to_string())).collect();

    for (name, value) in cookie_list.into_iter() {
        if name.as_str() == cookie_name {
            return Some(value);
        }
    }

    None

}
