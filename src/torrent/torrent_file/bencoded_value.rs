use serde::{Serialize, Deserialize};
use anyhow::{anyhow, Result};

use std::collections::BTreeMap;

use crate::peer::PeerAddress;

use super::Sha1Hash;

/// Represents a value in the Bencode format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BencodedValue {
    /// Represents a Bencoded dictionary (key-value pairs).
    Dict(BTreeMap<String, BencodedValue>),

    /// Represents a Bencoded list of values.
    List(Vec<BencodedValue>),
    
    /// Represents a Bencoded integer.
    Integer(i64),

    /// Represents a Bencoded byte string.
    ByteString(Vec<u8>),
    
    /// Represents a Bencoded byte string as a list of SHA-1 hashes.
    ByteSha1Hashes(Vec<Sha1Hash>),

    /// Represents a Bencoded byte string as a list of SHA-1 hashes.
    ByteAddresses(Vec<PeerAddress>),
}

impl BencodedValue {
    pub fn try_into_dict(&self) -> Result<&BTreeMap<String, BencodedValue>> {
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

    pub fn insert_into_dict(&mut self, key: String, value: BencodedValue) {
        if let BencodedValue::Dict(d) = self {
            d.insert(key, value);
        }
    }

    pub fn insert_into_list(&mut self, value: BencodedValue) {
        if let BencodedValue::List(l) = self {
            l.push(value);
        }
    }

    pub fn get_from_dict(&self, key: &str) -> Result<BencodedValue> {
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

        if ["announce", "info"].iter().any(|key| !dict.contains_key(*key)) {
            return false;
        }

        let info = match dict.get("info") {
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

        if ["name", "piece length", "pieces"].iter().any(|key| !info.contains_key(*key)) {
            return false;
        }

        if  ["files", "length"].iter().all(|key| !info.contains_key(*key)) ||
            ["files", "length"].iter().all(|key| info.contains_key(*key)) {
            return false;
        }

        if info.contains_key("files") {
            let files = match info.get("files") {
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
                    BencodedValue::Dict(d) => ["length", "path"].iter().all(|key| d.contains_key(*key)),
                    _ => false
                }
            });
        }

        true
    }
}

