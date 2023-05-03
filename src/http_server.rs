use tide::http::headers::{COOKIE, HeaderValue};
use tide::security::{CorsMiddleware, Origin};
pub use tide::*;

pub struct HttpServer<T> {
    app: Server<T>,
    server_addr: String,
    port: u16
}

impl<T: Clone + Send + Sync + 'static> HttpServer<T> {
    pub fn new(state: T, server_addr: String, port: u16, allow_origin: Option<Vec<String>>) -> Self {
        let mut app = tide::with_state(state);

        let cors = CorsMiddleware::new()
            .allow_methods(
                "GET, POST, PUT, DELETE, OPTIONS"
                    .parse::<HeaderValue>()
                    .unwrap(),
            )
            .allow_origin(Origin::from(allow_origin.unwrap_or(vec!["*".to_string()])))
            .allow_credentials(true)
            .allow_headers("*".parse::<HeaderValue>().unwrap())
            .expose_headers("*".parse::<HeaderValue>().unwrap());
        app.with(cors);

        Self {
            app,
            server_addr,
            port,
        }
    }

    pub fn app(&mut self) -> &mut Server<T> {
        &mut self.app
    }

    pub async fn run(self) -> tide::Result<()> {
        let addr = format!("{}:{}", self.server_addr, self.port);
        ::log::info!("start http server:{}", addr);
        self.app.listen(addr).await?;
        Ok(())
    }
}

pub fn get_param<'a, STATE>(req: &'a Request<STATE>, name: &str) -> tide::Result<&'a str> {
    req.param(name)
}

pub fn get_cookie<'a, STATE>(req: &'a Request<STATE>, cookie_name: &str) -> Option<String> {
    let cookie = req.header(COOKIE);
    if cookie.is_none() {
        return None;
    }

    //log::info!("cookie {}", cookie.unwrap().last().as_str());
    let cookie_list: Vec<_> = cookie.unwrap().last().as_str().split(";").collect();
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
