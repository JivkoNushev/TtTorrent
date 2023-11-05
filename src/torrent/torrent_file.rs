use std::collections::BTreeMap;
use percent_encoding::percent_encode;
use hex::encode;

pub mod parsers;
pub use parsers::{ parse_torrent_file, parse_to_torrent_file, parse_tracker_response };

use crate::torrent::peer::PeerAddress;
use crate::utils::read_file_as_bytes;

#[derive(Debug)]
pub struct TorrentFile {
    bencoded_dict: BencodedValue
} 
impl TorrentFile {
    pub fn new(torrent_file_name: &str) -> TorrentFile {
        let torrent_file: Vec<u8> = match read_file_as_bytes(&torrent_file_name) {
            Ok(data) => data,
            Err(e) => panic!("Error reading torrent file: {:?}", e)
        };
        TorrentFile { bencoded_dict: parse_torrent_file(&torrent_file) }
    }

    pub fn get_bencoded_dict(&self) -> &BencodedValue {
        &self.bencoded_dict
    }
}

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

    pub fn torrent_file_is_valid(&self) -> bool{
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

