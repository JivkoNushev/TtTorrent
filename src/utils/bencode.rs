use serde::{Serialize, Deserialize};
use anyhow::{anyhow, Result};

use std::collections::BTreeMap;

use crate::peer::PeerAddress;

use super::Sha1Hash;

mod parsing;

/// Represents a value in the Bencode format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BencodedValue {
    /// Represents a Bencoded dictionary (key-value pairs).
    Dict(BTreeMap<Vec<u8>, BencodedValue>),

    /// Represents a Bencoded list of values.
    List(Vec<BencodedValue>),
    
    /// Represents a Bencoded integer.
    Integer(i64),

    /// Represents a Bencoded byte string.
    ByteString(Vec<u8>),
    
    /// Represents a Bencoded byte string as a list of SHA-1 hashes.
    ByteSha1Hashes(Vec<Sha1Hash>),

    /// Represents a Bencoded list of peer addresses.
    ByteAddresses(Vec<PeerAddress>),
}

impl BencodedValue {
    pub fn from_bytes(bytes: &[u8]) -> Result<BencodedValue> {
        parsing::encode(&bytes)
    }

    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        parsing::decode(self)
    }

    pub fn try_into_dict(&self) -> Result<&BTreeMap<Vec<u8>, BencodedValue>> {
        match self {
            BencodedValue::Dict(d) => Ok(d),
            _ => Err(anyhow!("Trying to convert a non-bencoded dictionary"))
        }
    }

    pub fn try_into_integer(&self) -> Result<i64> {
        match self {
            BencodedValue::Integer(i) => Ok(*i),
            _ => Err(anyhow!("Trying to convert a non-bencoded integer"))
        }
    }

    pub fn try_into_list(&self) -> Result<&Vec<BencodedValue>> {
        match self {
            BencodedValue::List(l) => Ok(l),
            _ => Err(anyhow!("Trying to convert a non-bencoded list"))
        }
    }

    pub fn try_into_byte_string(&self) -> Result<&Vec<u8>> {
        match self {
            BencodedValue::ByteString(b) => Ok(b),
            _ => Err(anyhow!("Trying to convert a non-bencoded byte string"))
        }
    }

    pub fn try_into_byte_sha1_hashes(&self) -> Result<&Vec<Sha1Hash>> {
        match self {
            BencodedValue::ByteSha1Hashes(b) => Ok(b),
            _ => Err(anyhow!("Trying to convert a non-bencoded byte string"))
        }
    }

    pub fn insert_into_dict(&mut self, key: Vec<u8>, value: BencodedValue) {
        if let BencodedValue::Dict(d) = self {
            d.insert(key, value);
        }
    }

    pub fn insert_into_list(&mut self, value: BencodedValue) {
        if let BencodedValue::List(l) = self {
            l.push(value);
        }
    }

    pub fn get_from_dict(&self, key: &[u8]) -> Result<BencodedValue> {
        let dict = self.try_into_dict()?;

        match dict.get(key) {
            Some(value) => Ok(value.clone()),
            None => Err(anyhow!("Key not found in dictionary"))
        }
    }

    pub fn get_from_list(&self, index: usize) -> Result<BencodedValue> {
        let list = self.try_into_list()?;

        match list.get(index) {
            Some(value) => Ok(value.clone()),
            None => Err(anyhow!("Index out of bounds in list"))
        }
    }

    pub fn torrent_file_is_valid(&self) -> bool {
        let dict = match self.try_into_dict() {
            Ok(dict) => dict,
            Err(_) => return false
        };

        if [b"announce".to_vec(), b"info".to_vec()].iter().any(|key| !dict.contains_key(key)) {
            return false;
        }

        let info = match dict.get(&b"info".to_vec()) {
            Some(info) => {
                match info {
                    BencodedValue::Dict(info) => info,
                    _ => {
                        return false;
                    }
                }
            }
            None => {
                return false
            }
        };

        if [b"name".to_vec(), b"piece length".to_vec(), b"pieces".to_vec()].iter().any(|key| !info.contains_key(key)) {
            return false;
        }

        if  [b"files".to_vec(), b"length".to_vec()].iter().all(|key| !info.contains_key(key)) ||
            [b"files".to_vec(), b"length".to_vec()].iter().all(|key| info.contains_key(key)) {
            return false;
        }

        if info.contains_key(&b"files".to_vec()) {
            let files = match info.get(&b"files".to_vec()) {
                Some(files) => {
                    match files {
                        BencodedValue::List(files) => files,
                        _ => {
                            return false;
                        }
                    }
                }
                None => {
                    return false;
                }
            };

            return files
            .iter()
            .all(|file| {
                match file {
                    BencodedValue::Dict(d) => [&b"length".to_vec(), &b"path".to_vec()].iter().all(|key| d.contains_key(&key.to_vec())),
                    _ => false
                }
            });
        }

        true
    }
}