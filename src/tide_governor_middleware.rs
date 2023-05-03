
use governor::{
    clock::{Clock, DefaultClock},
    state::keyed::DefaultKeyedStateStore,
    Quota, RateLimiter,
};
use lazy_static::lazy_static;
use std::{
    convert::TryInto,
    error::Error,
    net::{IpAddr, SocketAddr},
    num::NonZeroU32,
    sync::Arc,
    time::Duration,
};
use std::fmt::Display;
use std::hash::Hash;
use tide::{
    http::StatusCode,
    log::{debug},
    utils::async_trait,
    Middleware, Next, Request, Response, Result,
};

lazy_static! {
    static ref CLOCK: DefaultClock = DefaultClock::default();
}

pub trait LimitKey: 'static + Default + Send + Sync {
    type KeyType: Display + Hash + Eq + Send + Sync + Clone;
    fn get_key<State: Clone + Send + Sync + 'static>(&self, req: &Request<State>) -> Result<Self::KeyType>;
}

#[derive(Default)]
pub struct IPAddrKey {

}

impl LimitKey for IPAddrKey {
    type KeyType = IpAddr;

    fn get_key<State: Clone + Send + Sync + 'static>(&self, req: &Request<State>) -> Result<Self::KeyType> {
        let remote = req.remote().ok_or_else(|| {
            tide::Error::from_str(
                StatusCode::InternalServerError,
                "failed to get request remote address",
            )
        })?;
        let remote: IpAddr = match remote.parse::<SocketAddr>() {
            Ok(r) => r.ip(),
            Err(_) => remote.parse()?,
        };
        log::debug!("remote: {}", remote);
        Ok(remote)
    }
}

#[derive(Debug, Clone)]
pub struct TideGovernorMiddleware<Key: LimitKey> {
    limit_key: Key,
    limiter: Arc<RateLimiter<Key::KeyType, DefaultKeyedStateStore<Key::KeyType>, DefaultClock>>,
}

impl<Key: LimitKey> TideGovernorMiddleware<Key> {
    pub fn new<T>(limit_key: Key, duration: Duration, times: T) -> Option<Self>
        where
            T: TryInto<NonZeroU32> {
        let times= times.try_into().map_or_else(|_| None, |v: NonZeroU32| Some(v))?;
        let replenish_interval_ns =
            duration.as_nanos() / times.get() as u128;
        Some(Self {
            limit_key,
            limiter: Arc::new(RateLimiter::<Key::KeyType, _, _>::keyed(Quota::with_period(
                Duration::from_nanos(replenish_interval_ns as u64),
            )?.allow_burst(times))),
        })
    }

    #[must_use]
    pub fn with_period<T>(duration: Duration, times: T) -> Option<Self>
        where
            T: TryInto<NonZeroU32> {
        let times= times.try_into().map_or_else(|_| None, |v: NonZeroU32| Some(v))?;
        let replenish_interval_ns =
            duration.as_nanos() / times.get() as u128;
        Some(Self {
            limit_key: Key::default(),
            limiter: Arc::new(RateLimiter::<Key::KeyType, _, _>::keyed(Quota::with_period(
                Duration::from_nanos(replenish_interval_ns as u64),
            )?.allow_burst(times))),
        })
    }

    pub fn per_second<T>(times: T) -> Result<Self>
        where
            T: TryInto<NonZeroU32>,
            T::Error: Error + Send + Sync + 'static,
    {
        Ok(Self {
            limit_key: Key::default(),
            limiter: Arc::new(RateLimiter::<Key::KeyType, _, _>::keyed(Quota::per_second(
                times.try_into()?,
            ))),
        })
    }

    pub fn per_minute<T>(times: T) -> Result<Self>
        where
            T: TryInto<NonZeroU32>,
            T::Error: Error + Send + Sync + 'static,
    {
        Ok(Self {
            limit_key: Key::default(),
            limiter: Arc::new(RateLimiter::<Key::KeyType, _, _>::keyed(Quota::per_minute(
                times.try_into()?,
            ))),
        })
    }

    pub fn per_hour<T>(times: T) -> Result<Self>
        where
            T: TryInto<NonZeroU32>,
            T::Error: Error + Send + Sync + 'static,
    {
        Ok(Self {
            limit_key: Key::default(),
            limiter: Arc::new(RateLimiter::<Key::KeyType, _, _>::keyed(Quota::per_hour(
                times.try_into()?,
            ))),
        })
    }
}

#[async_trait]
impl<State: Clone + Send + Sync + 'static, Key: LimitKey> Middleware<State> for TideGovernorMiddleware<Key> {
    async fn handle(&self, req: Request<State>, next: Next<'_, State>) -> tide::Result {
        let remote = self.limit_key.get_key(&req)?;

        match self.limiter.check_key(&remote) {
            Ok(_) => {
                debug!("allowing remote {}", remote);
                Ok(next.run(req).await)
            }
            Err(negative) => {
                let wait_time = negative.wait_time_from(CLOCK.now());
                let res = Response::builder(StatusCode::TooManyRequests)
                    .header(
                        tide::http::headers::RETRY_AFTER,
                        wait_time.as_secs().to_string(),
                    )
                    .build();
                debug!(
                    "blocking address {} for {} seconds",
                    remote,
                    wait_time.as_secs()
                );
                Ok(res)
            }
        }
    }
}
