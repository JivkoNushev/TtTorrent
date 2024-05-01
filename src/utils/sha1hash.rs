use std::fmt::{format, Display};

use percent_encoding::percent_encode;
use serde::{Serialize, Deserialize};
use anyhow::{anyhow, Result};

use crate::utils::UrlEncodable;

/// Represents a SHA-1 hash as an array of 20 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sha1Hash(pub [u8; 20]);

impl UrlEncodable for Sha1Hash {
    fn as_url_encoded(&self) -> String {
        percent_encode(&self.0, percent_encoding::NON_ALPHANUMERIC).to_string()
    }
}

impl Display for Sha1Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(self.0.as_slice()))
    }
}

impl Sha1Hash {
    pub fn new(hash: &[u8; 20]) -> Sha1Hash {
        Sha1Hash(*hash)
    }

    pub fn from_hex(hex: &str) -> Result<Sha1Hash> {
        let bytes = hex::decode(hex)?;
        if bytes.len() != 20 {
            return Err(anyhow!("invalid sha1 hash length"));
        }
        Ok(Sha1Hash(bytes.try_into().unwrap()))
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

use sha1::Digest;
pub fn sha1_hash(value: Vec<u8>) -> Sha1Hash {
    let mut hasher = sha1::Sha1::new();
    hasher.update(value);
    let hash = hasher.finalize();
    let hash = hash.as_slice().try_into().unwrap(); // sha1 hash is always 20 bytes in this case

    Sha1Hash::new(hash)
}   
