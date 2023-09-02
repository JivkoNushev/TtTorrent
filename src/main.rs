use std::collections::HashMap;
use std::fs::File;
use std::hash::Hash;
use std::io::Read;
use std::fmt::Error;

use dict::Dict;

#[derive(Debug)]
enum TorrentFileValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    Bytes(Vec<u8>),
    List(Vec<TorrentFileValue>),
    Dictionary(HashMap<String, TorrentFileValue>),
}

impl TorrentFileValue {
    fn from_string(s: String) -> TorrentFileValue {
        TorrentFileValue::String(s)
    }

    fn pop_string(&mut self) -> String {
        match self {
            TorrentFileValue::String(s) => s.clone(),
            _ => panic!("Not a string"),
        }
    }

    fn from_integer(i: i64) -> TorrentFileValue {
        TorrentFileValue::Integer(i)
    }

    fn pop_integer(&mut self) -> i64 {
        match self {
            TorrentFileValue::Integer(i) => *i,
            _ => panic!("Not an integer"),
        }
    }

    fn from_boolean(b: bool) -> TorrentFileValue {
        TorrentFileValue::Boolean(b)
    }

    fn pop_boolean(&mut self) -> bool {
        match self {
            TorrentFileValue::Boolean(b) => *b,
            _ => panic!("Not a boolean"),
        }
    }

    fn from_bytes(b: Vec<u8>) -> TorrentFileValue {
        TorrentFileValue::Bytes(b)
    }

    fn pop_bytes(&mut self) -> &mut Vec<u8> {
        match self {
            TorrentFileValue::Bytes(b) => b,
            _ => panic!("Not a byte string"),
        }
    }

    fn from_list(l: Vec<TorrentFileValue>) -> TorrentFileValue {
        TorrentFileValue::List(l)
    }

    fn pop_list(&mut self) -> Option<&mut Vec<TorrentFileValue>> {
        match self {
            TorrentFileValue::List(l) => Some(l),
            _ => None,
        }
    }

    fn from_dictionary(d: HashMap<String, TorrentFileValue>) -> TorrentFileValue {
        TorrentFileValue::Dictionary(d)
    }

    fn pop_dict(&mut self) -> Option<&mut HashMap<String, TorrentFileValue>> {
        match self {
            TorrentFileValue::Dictionary(d) => Some(d),
            _ => None,
        }
    }

    fn add_to_dict(&mut self, key: String, value: TorrentFileValue) {
        match self {
            TorrentFileValue::Dictionary(d) => {
                d.insert(key, value);
            }
            _ => panic!("Not a dictionary"),
        }
    }

}


fn main() {
    let mut torrent_file = File::open("kamasutra.torrent").unwrap();
    let mut bencoded_string: Vec<u8> = Vec::new();

    torrent_file.read_to_end(&mut bencoded_string);

    let torrent_file_value = parse_torrent_file(bencoded_string).unwrap();

    println!("{:?}", torrent_file_value);
}