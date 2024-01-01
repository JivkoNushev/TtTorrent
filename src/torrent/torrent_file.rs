use anyhow::{Result, Context, anyhow};
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

    pub fn get_info_hash(bencoded_dict: &BencodedValue) -> Result<Sha1Hash> {
        let info_dict = bencoded_dict.get_from_dict("info")?;
        match info_dict {
            BencodedValue::Dict(_) => {
                let bencoded_info_dict = TorrentParser::parse_to_torrent_file(&info_dict)?;
                Ok(sha1_hash(bencoded_info_dict))
            },
            _ => Err(anyhow!("Invalid dictionary in info key when getting the tracker params"))
        }
    }

    pub fn get_piece_hash(&self, piece_index: usize) -> Result<Sha1Hash> {
        let info_dict = self.bencoded_dict.get_from_dict("info")?;
        let pieces = info_dict.get_from_dict("pieces")?;

        match pieces {
            BencodedValue::ByteSha1Hashes(pieces) => {
                match pieces.get(piece_index) {
                    Some(piece_hash) => Ok(piece_hash.clone()),
                    None => Err(anyhow!("Invalid piece index in info dict ref in torrent file: {:?}", self.bencoded_dict))
                }
            },
            _ => Err(anyhow!("Invalid pieces key in info dict ref in torrent file: {:?}", self.bencoded_dict))
        }
    }

    pub fn get_piece_length(&self) -> Result<usize> {
        let info_dict = self.bencoded_dict.get_from_dict("info")?;

        let piece_length = info_dict.get_from_dict("piece length")?;

        match piece_length {
            BencodedValue::Integer(piece_length) => Ok(piece_length as usize),
            _ => Err(anyhow!("Invalid piece length key in info dict ref in torrent file: {:?}", self.bencoded_dict))
        }
    }

    pub fn get_torrent_length(&self) -> Result<u64> {
        let torrent_dict = self.get_bencoded_dict_ref().try_into_dict()?;
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => return Err(anyhow!("Could not get info dict from torrent file ref: {:?}", self.bencoded_dict))
        };

        let mut total_size: u64 = 0;

        if let Ok(files_dict) = info_dict.get_from_dict("files") {
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
            let file_length = info_dict.get_from_dict("length")?;
            let file_length = match file_length {
                BencodedValue::Integer(file_length) => file_length,
                _ => return Err(anyhow!("Could not get file size from info dict ref in torrent file"))
            };

            total_size = file_length as u64;
        }
        
        Ok(total_size)
    }
}

// TorrentFile methods
impl TorrentFile {
    pub async fn new(torrent_file_name: &str) -> Result<TorrentFile> {
        let torrent_file = read_file_as_bytes(torrent_file_name).await.context("couldn't read file as bytes")?;
        let bencoded_dict = TorrentParser::parse_torrent_file(&torrent_file)?;

        Ok(TorrentFile { bencoded_dict })
    }
}

