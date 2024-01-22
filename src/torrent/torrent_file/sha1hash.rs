use percent_encoding::percent_encode;
use serde::{Serialize, Deserialize};

use crate::utils::UrlEncodable;

/// Represents a SHA-1 hash as an array of 20 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sha1Hash(pub [u8; 20]);

impl UrlEncodable for Sha1Hash {
    fn as_url_encoded(&self) -> String {
        percent_encode(&self.0, percent_encoding::NON_ALPHANUMERIC).to_string()
    }
}

impl Sha1Hash {
    pub fn new(hash: &[u8; 20]) -> Sha1Hash {
        Sha1Hash(*hash)
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }

    pub fn to_string(&self) -> String {
        String::from_utf8_lossy(self.0.as_slice()).to_string()
    }
}
