use std::collections::BTreeMap;

use crate::peer::PeerAddress;

use super::Sha1Hash;

/// Represents a value in the Bencode format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BencodedValue {
    /// Represents a Bencoded integer.
    Integer(i128),

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
}

impl BencodedValue {
    pub fn try_into_dict(&self) -> Option<&BTreeMap<String, BencodedValue>> {
        if let BencodedValue::Dict(d) = self {
            Some(d)
        }
        else {
            None
        }
    }

    pub fn try_into_integer(&self) -> Option<&i128> {
        if let BencodedValue::Integer(i) = self {
            Some(i)
        }
        else {
            None
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

    // TODO: maybe return Result types for the getters
    pub fn get_from_dict(&self, key: &str) -> Option<BencodedValue> {
        if let BencodedValue::Dict(d) = self {
            if let Some(value) = d.get(key) {
                return Some(value.clone());
            }
            None
        }
        else {
            panic!("Trying to get a value from a non-bencoded dictionary");
        }
    }

    pub fn get_from_list(&self, index: usize) -> Option<BencodedValue> {
        if let BencodedValue::List(l) = self {
            if let Some(value) = l.get(index) {
                return Some(value.clone());
            }
            None
        }
        else {
            panic!("Trying to get a value from a non-benocoded list");
        }
    }

    pub fn torrent_file_is_valid(&self) -> bool {
        if let BencodedValue::Dict(d) = self {
            if ["announce", "info"].iter().any(|key| !d.contains_key(*key)) {
                false
            }
            else {
                if let BencodedValue::Dict(info) = d.get("info").unwrap() {
                    if ["name", "piece length", "pieces"].iter().any(|key| !info.contains_key(*key)) {
                        false
                    }
                    else {
                        if ["files", "length"].iter().all(|key| !info.contains_key(*key)) {
                            false
                        }
                        else if ["files", "length"].iter().all(|key| info.contains_key(*key)) {
                            false
                        }
                        else {
                            if info.contains_key("files") {
                                if let BencodedValue::List(l) = info.get("files").unwrap() {
                                    l.iter().all(|file| {
                                        if let BencodedValue::Dict(d) = file {
                                            ["length", "path"].iter().all(|key| d.contains_key(*key))
                                        }
                                        else {
                                            panic!("Trying to validate a non-bencoded dictionary in \"files\"");
                                        }
                                    })
                                }
                                else {
                                    panic!("Trying to validate a non-bencoded list named \"files\"")
                                }
                            }
                            else {
                                true
                            }
                        }
                    }
                }
                else {
                    panic!("Trying to validate a non-bencoded dictionary named \"info\"")
                }
            }
        }
        else {
            panic!("Trying to validate a non-bencoded dictionary");
        }
    }
}

