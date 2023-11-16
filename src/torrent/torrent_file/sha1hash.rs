use percent_encoding::percent_encode;

use crate::utils::UrlEncodable;

/// Represents a SHA-1 hash as an array of 20 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sha1Hash(pub [u8; 20]);

impl Sha1Hash {
    pub fn new(hash: &[u8]) -> Sha1Hash {
        if hash.len() != 20 {
            panic!("Hash must be 20 bytes long");
        }

        Sha1Hash(hash.try_into().unwrap())
    }

    pub fn get_hash_ref(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn as_hex(&self) -> String {
        let hex_string = hex::encode(&self.0);
        hex_string
    }

    pub fn as_string(&self) -> String {
        String::from_utf8_lossy(self.0.as_slice()).to_string()
    }
}

impl UrlEncodable for Sha1Hash {
    fn as_url_encoded(&self) -> String {
        percent_encode(&self.0, percent_encoding::NON_ALPHANUMERIC).to_string()
    }
}