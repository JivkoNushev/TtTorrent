
// pub mod parsers;
// pub use parsers::{ parse_torrent_file, parse_to_torrent_file, parse_tracker_response };

// use crate::torrent::peer::PeerAddress;
use crate::utils::{read_file_as_bytes, sha1_hash};

pub mod bencoded_value;
pub mod sha1hash;

pub use bencoded_value::BencodedValue;
pub use sha1hash::Sha1Hash;

use super::TorrentParser;

#[derive(Debug, Clone)]
pub struct TorrentFile {
    bencoded_dict: BencodedValue,
} 
impl TorrentFile {
    pub async fn new(torrent_file_name: String) -> TorrentFile {
        let torrent_file: Vec<u8> = match read_file_as_bytes(&torrent_file_name) {
            Ok(data) => data,
            Err(e) => panic!("Error reading torrent file: {:?}", e)
        };

        let bencoded_dict = TorrentParser::parse_torrent_file(&torrent_file).await;

        TorrentFile { bencoded_dict }
    }

    pub fn get_bencoded_dict_ref(&self) -> &BencodedValue {
        &self.bencoded_dict
    }

    pub async fn get_info_hash(bencoded_dict: &BencodedValue) -> Option<Sha1Hash> {
        if let Some(dict) = bencoded_dict.get_from_dict("info") {
            if let BencodedValue::Dict(_) = dict {
                let bencoded_info_dict = TorrentParser::parse_to_torrent_file(&dict).await;
                return Some(sha1_hash(bencoded_info_dict));
            }
            else {
                panic!("Invalid dictionary in info key when getting the tracker params");
            }
        }

        None
    }
}

