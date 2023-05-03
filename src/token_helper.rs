use chrono::{DateTime, Utc};
use jsonwebtoken::{Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
pub use jsonwebtoken::*;

pub type TokenResult<T> = errors::Result<T>;

#[derive(Serialize, Deserialize)]
struct Payload<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>, //签发人
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<u64>, //过期时间
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<String>, //主题
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>, //受众
    #[serde(skip_serializing_if = "Option::is_none")]
    nbf: Option<u64>, //生效时间
    #[serde(skip_serializing_if = "Option::is_none")]
    iat: Option<u64>, //签发时间
    #[serde(skip_serializing_if = "Option::is_none")]
    jti: Option<u64>, //编号
    data: T,
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

}
