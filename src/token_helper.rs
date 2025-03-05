use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
pub use jsonwebtoken::*;
pub use chrono;

pub type TokenResult<T> = errors::Result<T>;

#[derive(Serialize, Deserialize)]
pub struct Payload<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>, //签发人
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<u64>, //过期时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>, //主题
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>, //受众
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<u64>, //生效时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iat: Option<u64>, //签发时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<u64>, //编号
    pub data: T,
}

impl<T> Payload<T> {
    pub fn is_expire(&self, interval: Duration) -> bool {
        if let Some(exp) = self.exp {
            let now = Utc::now().timestamp();
            exp < now as u64 + interval.as_secs()
        } else {
            false
        }
    }
}

pub struct JsonWebToken;

impl JsonWebToken {
    pub fn encode<T: Serialize>(alg: Algorithm, data: T, expired_at: DateTime<Utc>, key: &EncodingKey) -> TokenResult<String> {
        let header = Header::new(alg);

        let payload = Payload {
            iss: None,
            exp: Some(expired_at.timestamp() as u64),
            sub: None,
            aud: None,
            nbf: None,
            iat: None,
            jti: None,
            data,
        };
        jsonwebtoken::encode(&header, &payload, key)
    }

    pub fn decode<T: for<'a> Deserialize<'a>>(token: &str, key: &DecodingKey) -> TokenResult<T> {
        let header = jsonwebtoken::decode_header(token)?;
        let mut val = Validation::new(header.alg);
        val.validate_exp = true;
        let token_data: TokenData<Payload<T>> = jsonwebtoken::decode(token, key, &val)?;
        Ok(token_data.claims.data)
    }

    pub fn decode_payload<T: for<'a> Deserialize<'a>>(token: &str, key: &DecodingKey) -> TokenResult<Payload<T>> {
        let header = jsonwebtoken::decode_header(token)?;
        let mut val = Validation::new(header.alg);
        val.validate_exp = true;
        let token_data: TokenData<Payload<T>> = jsonwebtoken::decode(token, key, &val)?;
        Ok(token_data.claims)
    }
}

pub struct JWTBuilder<T: Serialize> {
    payload: Payload<T>
}

impl<T: Serialize> JWTBuilder<T> {
    pub fn new(data: T) -> Self {
        Self {
            payload: Payload {
                iss: None,
                exp: None,
                sub: None,
                aud: None,
                nbf: None,
                iat: None,
                jti: None,
                data,
            }
        }
    }

    pub fn iss(mut self, iss: String) -> Self {
        self.payload.iss = Some(iss);
        self
    }

    pub fn exp(mut self, exp: DateTime<Utc>) -> Self {
        self.payload.exp = Some(exp.timestamp() as u64);
        self
    }

    pub fn sub(mut self, sub: String) -> Self {
        self.payload.sub = Some(sub);
        self
    }

    pub fn aud(mut self, aud: String) -> Self {
        self.payload.aud = Some(aud);
        self
    }

    pub fn nbf(mut self, nbf: DateTime<Utc>) -> Self {
        self.payload.nbf = Some(nbf.timestamp() as u64);
        self
    }

    pub fn iat(mut self, iat: DateTime<Utc>) -> Self {
        self.payload.iat = Some(iat.timestamp() as u64);
        self
    }

    pub fn jti(mut self, jti: u64) -> Self {
        self.payload.jti = Some(jti);
        self
    }

    pub fn build(self, alg: Algorithm, key: &EncodingKey) -> TokenResult<String> {
        let header = Header::new(alg);
        jsonwebtoken::encode(&header, &self.payload, key)
    }
}
