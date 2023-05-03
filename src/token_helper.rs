use chrono::{DateTime, Utc};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use jsonwebtoken::{Header, TokenData, Validation};
use jsonwebtoken::errors::ErrorKind;
use serde::{Deserialize, Serialize};

pub type Algorithm = jsonwebtoken::Algorithm;
pub type EncodingKey = jsonwebtoken::EncodingKey;
pub type DecodingKey = jsonwebtoken::DecodingKey;

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
    pub fn encode<T: Serialize>(alg: Algorithm, data: T, expired_at: DateTime<Utc>, key: &EncodingKey) -> BuckyResult<String> {
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
        jsonwebtoken::encode(&header, &payload, key).map_err(|e| {
            let msg = format!("encode token failed {}", e);
            log::error!("{}", msg);
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })
    }

    pub fn decode<T: for<'a> Deserialize<'a>>(token: &str, key: &DecodingKey) -> BuckyResult<T> {
        let header = jsonwebtoken::decode_header(token).map_err(|e| {
            let msg = format!("decode header failed {}", e);
            log::error!("{}", msg);
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;
        let mut val = Validation::new(header.alg);
        val.validate_exp = true;
        let token_data: TokenData<Payload<T>> = jsonwebtoken::decode(token, key, &val).map_err(|e| {
            let msg = format!("decode token failed {}", e);
            log::error!("{}", msg);
            if e.kind() == &ErrorKind::ExpiredSignature {
                BuckyError::new(BuckyErrorCode::Expired, msg)
            } else {
                BuckyError::new(BuckyErrorCode::Failed, msg)
            }
        })?;
        Ok(token_data.claims.data)
    }

}
