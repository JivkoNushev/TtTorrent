use std::collections::BTreeMap;

use crate::peer::PeerAddress;

use super::Sha1Hash;

/// Represents a value in the Bencode format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BencodedValue {
    /// Represents a Bencoded integer.
    Integer(i64),

    /// Represents a Bencoded byte string as a list of SHA-1 hashes.
    ByteSha1Hashes(Vec<Sha1Hash>),

    /// Represents a Bencoded byte string as a list of SHA-1 hashes.
    ByteAddresses(Vec<PeerAddress>),

    /// Represents a Bencoded list of values.
    List(Vec<BencodedValue>),

    /// Represents a Bencoded dictionary (key-value pairs).
    Dict(BTreeMap<String, BencodedValue>),

    /// Represents a Bencoded string.
    String(String),

    /// Represents a Bencoded byte string.
    ByteString(Vec<u8>),
}

impl BencodedValue {
    pub fn try_into_dict(&self) -> Option<&BTreeMap<String, BencodedValue>> {
        match self {
            BencodedValue::Dict(d) => Some(d),
            _ => None
        }
    }

    pub fn try_into_integer(&self) -> Option<&i64> {
        match self {
            BencodedValue::Integer(i) => Some(i),
            _ => None
        }
    }

    pub fn try_into_list(&self) -> Option<&Vec<BencodedValue>> {
        match self {
            BencodedValue::List(l) => Some(l),
            _ => None
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

    pub fn get_from_dict(&self, key: &str) -> Option<BencodedValue> {
        let dict = match self.try_into_dict() {
            Some(dict) => dict,
            None => panic!("Trying to get a value from a non-bencoded dictionary")
        };

        match dict.get(key) {
            Some(value) => Some(value.clone()),
            None => None
        }
    }

    pub fn get_from_list(&self, index: usize) -> Option<BencodedValue> {
        let list = match self.try_into_list() {
            Some(list) => list,
            None => panic!("Trying to get a value from a non-bencoded list")
        };

        match list.get(index) {
            Some(value) => Some(value.clone()),
            None => None
        }
    }

    pub fn torrent_file_is_valid(&self) -> bool {
        let dict = match self.try_into_dict() {
            Some(dict) => dict,
            None => panic!("Trying to validate a non-bencoded dictionary")
        };

        if ["announce", "info"].iter().any(|key| !dict.contains_key(*key)) {
            return false;
        }

        let info = match dict.get("info") {
            Some(info) => {
                match info {
                    BencodedValue::Dict(info) => info,
                    _ => panic!("Trying to validate a non-bencoded dictionary named \"info\"")
                }
            }
            None => panic!("Trying to validate a non-bencoded dictionary named \"info\"")
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
                        _ => panic!("Trying to validate a non-bencoded list named \"files\"")
                    }
                }
                None => panic!("Trying to validate a non-bencoded list named \"files\"")
            };

            return files
            .iter()
            .all(|file| {
                match file {
                    BencodedValue::Dict(d) => ["length", "path"].iter().all(|key| d.contains_key(*key)),
                    _ => panic!("Trying to validate a non-bencoded dictionary in \"files\"")
                }
            });
        }

        true
    }
}

