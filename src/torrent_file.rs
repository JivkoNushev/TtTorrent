use std::collections::HashMap;
use std::io::{Result, Read};
use urlencoding::encode as urlencode;
use percent_encoding::{utf8_percent_encode, percent_encode};

use hex::encode;

pub mod parsers;

pub use parsers::{ parse_torrent_file, parse_to_torrent_file };

/// Represents a SHA-1 hash as an array of 20 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sha1Hash([u8; 20]);

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

    pub fn as_hex(&self) -> String{
        let hex_string = encode(&self.0);
        hex_string
    }

    pub fn as_string(&self) -> String{
        String::from_utf8_lossy(self.0.as_slice()).to_string()
    }

    pub fn as_url_encoded(&self) -> String {
        let s = self.0;
        percent_encode(&s, percent_encoding::NON_ALPHANUMERIC).to_string()
    }
}


/// Represents a value in the Bencode format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BencodedValue {
    /// Represents a Bencoded integer.
    Integer(i128),

    /// Represents a Bencoded byte string as a list of SHA-1 hashes.
    ByteString(Vec<Sha1Hash>),

    /// Represents a Bencoded list of values.
    List(Vec<BencodedValue>),

    /// Represents a Bencoded dictionary (key-value pairs).
    Dict(HashMap<String, BencodedValue>),

    /// Represents a Bencoded string.
    String(String),
}

impl BencodedValue {
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

    pub fn get_from_dict(&self, key: &str) -> BencodedValue {
        if let BencodedValue::Dict(d) = self {
            if let Some(value) = d.get(key) {
                value.clone()
            }
            else {
                panic!("Trying to get a value from a bencoded dictionary with a key that does not exist");
            }
        }
        else {
            panic!("Trying to get a value from a non-bencoded dictionary");
        }
    }

    pub fn get_from_list(&self, index: usize) -> BencodedValue {
        if let BencodedValue::List(l) = self {
            if let Some(value) = l.get(index) {
                value.clone()
            }
            else {
                panic!("Trying to get a value from a bencoded list with an index out of bounds");
            }
        }
        else {
            panic!("Trying to get a value from a non-benocoded list");
        }
    }

    pub fn is_valid_torrent_file(&self) -> bool{
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

pub fn read_file_as_bytes(path: &str) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut file = std::fs::File::open(path)?;

    file.read_to_end(&mut buf)?;

    Ok(buf)
}