use std::time::{Duration, SystemTime, UNIX_EPOCH};
use base58::ToBase58;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use sha2::Digest;

#[derive(Serialize, Deserialize, Clone)]
pub struct SignedData<T> {
    __signature___: String,
    __timestamp___: u64,
    #[serde(flatten)]
    data: T,
}

impl<T: Serialize + for<'a> Deserialize<'a> + Clone> SignedData<T> {
    pub fn new(data: T) -> Self {
        Self {
            __signature___: "".to_string(),
            __timestamp___: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            data
        }
    }

    pub fn sign(&mut self, key: &str) {
        self.__signature___ = self.create_signature(key);
    }

    fn create_signature(&self, key: &str) -> String {
        let value = serde_json::to_value(self).unwrap();
        let list = value.as_object().unwrap().iter()
            .sorted_by(|(k1, _), (k2, _)| Ord::cmp(k1.as_str(), k2.as_ref()))
            .filter(|(k, _)| k.as_str() != "__signature___")
            .map(|(k, v)| {
                if v.is_string() {
                    format!("{}={}", k, v.as_str().unwrap())
                } else {
                    format!("{}={}", k, v)
                }
            })
            .join("&");
        let mut hash = sha2::Sha256::new();
        hash.update(format!("{}&__key___={}", list, key).as_bytes());
        hash.finalize().to_base58()
    }

    pub fn verify_signature(&self, key: &str) -> bool {
        let signature = self.create_signature(key);
        if signature != self.__signature___ {
            false
        } else {
            true
        }
    }

    pub fn verify(&self, key: &str, valid_time: Duration) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        if now - self.__timestamp___ > valid_time.as_secs() {
            return false;
        }

        self.verify_signature(key)
    }

    pub fn to_data(self) -> T {
        self.data
    }
}

#[cfg(test)]
mod test {
    use std::thread::sleep;
    use super::*;

    #[derive(Serialize, Deserialize, Clone)]
    struct TestData {
        name: String,
        age: u32,
    }

    #[test]
    fn test_signed_data() {
        let mut data = SignedData::new(TestData {
            name: "test".to_string(),
            age: 18,
        });
        data.sign("test_key");
        assert_eq!(data.verify_signature("test_key"), true);
        sleep(Duration::from_secs(4));
        assert_eq!(data.verify("test_key", Duration::from_secs(10)), true);
        assert_eq!(data.verify("test_key", Duration::from_secs(1)), false);
        assert_eq!(data.to_data().name, "test");
    }
}
