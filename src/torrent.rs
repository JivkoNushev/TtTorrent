use std::collections::{LinkedList, BTreeMap};

use tokio::sync::Mutex;

pub mod torrent_file;
pub mod torrent_parser;

pub use torrent_file::{TorrentFile, Sha1Hash};
pub use torrent_parser::TorrentParser;
pub use self::torrent_file::BencodedValue;

#[derive(Debug)]
pub struct Torrent {
    pub torrent_name: String,
    pub torrent_file: TorrentFile,
    pub info_hash: Sha1Hash,
    pub pieces_left: Vec<u32>,
}

impl Torrent {
    pub async fn new(torrent_name: String) -> Torrent {
        let torrent_file = TorrentFile::new(&torrent_name).await;

        let info_hash = match TorrentFile::get_info_hash(torrent_file.get_bencoded_dict_ref()) {
            Some(info_hash) => info_hash,
            None => panic!("Could not get info hash from torrent file: {}", torrent_name)
        };

        let pieces_left = {
            let torrent_dict = match torrent_file.get_bencoded_dict_ref().try_into_dict() {
                Some(torrent_dict) => torrent_dict,
                None => panic!("Could not get torrent dict ref from torrent file: {}", torrent_name)
            };
            let info_dict = match torrent_dict.get("info") {
                Some(info_dict) => info_dict,
                None => panic!("Could not get info dict from torrent file ref: {}", torrent_name)
            };
            let pieces = match info_dict.get_from_dict("pieces") {
                Some(pieces) => pieces,
                None => panic!("Could not get pieces from info dict ref in torrent file: {}", torrent_name)
            };

            if let BencodedValue::ByteSha1Hashes(pieces) = pieces {
                let mut pieces_left = Vec::new();
                for i in 0..pieces.len() {
                    pieces_left.push(i as u32);
                }
                pieces_left
            }
            else {
                panic!("Could not get pieces from info dict ref in torrent file: {}", torrent_name)
            }
        };

        Torrent {
            torrent_name,
            torrent_file,
            info_hash,
            pieces_left,
        }
    }

    pub fn get_info_hash_ref(&self) -> &Sha1Hash {
        &self.info_hash
    }

    pub fn get_piece_length(&self, piece_index: usize) -> i128 {
        let torrent_dict = match self.torrent_file.get_bencoded_dict_ref().try_into_dict() {
            Some(torrent_dict) => torrent_dict,
            None => panic!("Could not get torrent dict ref from torrent file: {}", self.torrent_name)
        };
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {}", self.torrent_name)
        };

        let piece_length = match info_dict.get_from_dict("piece length") {
            Some(piece_length) => piece_length.try_into_integer().unwrap().clone(),
            None => panic!("Could not get piece length from info dict ref in torrent file: {}", self.torrent_name)
        };

        if piece_index as u32 == self.get_piece_count() - 1 {
            self.get_file_size() as i128 % piece_length
        }
        else {
            piece_length
        }
    }

    pub fn get_piece_count(&self) -> u32 {
        let torrent_dict = match self.torrent_file.get_bencoded_dict_ref().try_into_dict() {
            Some(torrent_dict) => torrent_dict,
            None => panic!("Could not get torrent dict ref from torrent file: {}", self.torrent_name)
        };
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {}", self.torrent_name)
        };
        let pieces = match info_dict.get_from_dict("pieces") {
            Some(pieces) => pieces,
            None => panic!("Could not get pieces from info dict ref in torrent file: {}", self.torrent_name)
        };

        if let BencodedValue::ByteSha1Hashes(pieces) = pieces {
            pieces.len() as u32
        }
        else {
            panic!("Could not get pieces from info dict ref in torrent file: {}", self.torrent_name)
        }
    }

    pub fn get_file_size(&self) -> u32 {
        let torrent_dict = match self.torrent_file.get_bencoded_dict_ref().try_into_dict() {
            Some(torrent_dict) => torrent_dict,
            None => panic!("Could not get torrent dict ref from torrent file: {}", self.torrent_name)
        };
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {}", self.torrent_name)
        };

        // if has files, sum all file sizes
        if let Some(files) = info_dict.get_from_dict("files") {
            if let BencodedValue::List(files) = files {
                let mut total_size = 0;
                for file in files {
                    if let BencodedValue::Dict(file) = file {
                        if let Some(length) = file.get("length") {
                            if let BencodedValue::Integer(length) = length {
                                total_size += length;
                            }
                        }
                    }
                }
                return total_size as u32;
            }
        }
        // else, get length
        if let Some(length) = info_dict.get_from_dict("length") {
            if let BencodedValue::Integer(length) = length {
                return length as u32;
            }
        }

        0
    }

    pub fn get_files(&self) -> Vec<BTreeMap<String, BencodedValue>> {
        let torrent_dict = match self.torrent_file.get_bencoded_dict_ref().try_into_dict() {
            Some(torrent_dict) => torrent_dict,
            None => panic!("Could not get torrent dict ref from torrent file: {}", self.torrent_name)
        };
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {}", self.torrent_name)
        };
        
        let mut files = Vec::new();
        
        if let Some(files_list) = info_dict.get_from_dict("files") {
            if let BencodedValue::List(files_list) = files_list {
                for file in files_list {
                    if let BencodedValue::Dict(file) = file {
                        files.push(file.clone());
                    }
                }
            }
        }
        else {
            if let Some(file) = info_dict.get_from_dict("name") {
                if let BencodedValue::String(file) = file {
                    if let BencodedValue::Integer(length) = info_dict.get_from_dict("length").unwrap() {
                        let mut file_dict = BTreeMap::new();
                        file_dict.insert("path".to_string(), BencodedValue::String(file.clone()));
                        file_dict.insert("length".to_string(), BencodedValue::Integer(length.clone()));
                        files.push(file_dict);
                    }
                }
            }
        }

        files
    }
}
