use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};

use crate::utils::{read_file_as_bytes, sha1_hash};
use super::TorrentParser;

pub mod bencoded_value;
pub use bencoded_value::BencodedValue;

pub mod sha1hash;
pub use sha1hash::Sha1Hash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFile {
    bencoded_dict: BencodedValue,
}

// TorrentFile getters
impl TorrentFile {
    pub fn get_bencoded_dict_ref(&self) -> &BencodedValue {
        &self.bencoded_dict
    }

    pub fn get_info_hash(bencoded_dict: &BencodedValue) -> Option<Sha1Hash> {
        let info_dict = match bencoded_dict.get_from_dict("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {:?}", bencoded_dict)
        };

        if let BencodedValue::Dict(_) = info_dict {
            let bencoded_info_dict = TorrentParser::parse_to_torrent_file(&info_dict);
            return Some(sha1_hash(bencoded_info_dict));
        }
        else {
            panic!("Invalid dictionary in info key when getting the tracker params");
        }
    }

    pub fn get_piece_hash(&self, piece_index: usize) -> Option<Sha1Hash> {
        let info_dict = match self.bencoded_dict.get_from_dict("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {:?}", self.bencoded_dict)
        };

        let pieces = match info_dict.get_from_dict("pieces") {
            Some(pieces) => pieces,
            None => panic!("Could not get pieces from info dict ref in torrent file: {:?}", self.bencoded_dict)
        };

        match pieces {
            BencodedValue::ByteSha1Hashes(pieces) => {
                match pieces.get(piece_index) {
                    Some(piece_hash) => Some(piece_hash.clone()),
                    None => None
                }
            },
            _ => panic!("Invalid pieces key in info dict ref in torrent file: {:?}", self.bencoded_dict)
        }
    }

    pub fn get_piece_length(&self) -> usize {
        let info_dict = match self.bencoded_dict.get_from_dict("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {:?}", self.bencoded_dict)
        };

        let piece_length = match info_dict.get_from_dict("piece length") {
            Some(piece_length) => piece_length,
            None => panic!("Could not get piece length from info dict ref in torrent file: {:?}", self.bencoded_dict)
        };

        match piece_length {
            BencodedValue::Integer(piece_length) => piece_length as usize,
            _ => panic!("Invalid piece length key in info dict ref in torrent file: {:?}", self.bencoded_dict)
        }
    }

    pub fn get_torrent_length(&self) -> u64 {
        let torrent_dict = match self.get_bencoded_dict_ref().try_into_dict() {
            Some(torrent_dict) => torrent_dict,
            None => panic!("Could not get torrent dict ref from torrent file")
        };
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref")
        };

        let mut total_size: u64 = 0;

        if let Some(files_dict) = info_dict.get_from_dict("files") {
            let files = match files_dict {
                BencodedValue::List(files) => files,
                _ => panic!("Could not get file size from info dict ref in torrent file")
            };

            for file in files {
                let file = match file {
                    BencodedValue::Dict(file) => file,
                    _ => panic!("Could not get file size from info dict ref in torrent file")
                };

                let length = match file.get("length") {
                    Some(length) => length,
                    None => panic!("Could not get file size from info dict ref in torrent file")
                };

                let length = match length {
                    BencodedValue::Integer(length) => length,
                    _ => panic!("Could not get file size from info dict ref in torrent file")
                };

                total_size += *length as u64;
            }
        }
        else { 
            let file_length = match info_dict.get_from_dict("length") {
                Some(file_length) => file_length,
                None => panic!("Could not get file size from info dict ref in torrent file")
            }; 
            let file_length = match file_length {
                BencodedValue::Integer(file_length) => file_length,
                _ => panic!("Could not get file size from info dict ref in torrent file")
            };

            total_size = file_length as u64;
        }
        
        total_size
    }
}

// TorrentFile methods
impl TorrentFile {
    pub async fn new(torrent_file_name: &str) -> Result<TorrentFile> {
        let torrent_file = read_file_as_bytes(torrent_file_name).await.context("couldn't read file as bytes")?;
        let bencoded_dict = TorrentParser::parse_torrent_file(&torrent_file);

        Ok(TorrentFile { bencoded_dict })
    }
}

