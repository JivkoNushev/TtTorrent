use std::collections::BTreeMap;

pub mod torrent_file;
pub use torrent_file::{BencodedValue, TorrentFile, Sha1Hash};

pub mod torrent_parser;
pub use torrent_parser::TorrentParser;

#[derive(Debug)]
pub struct Torrent {
    pub torrent_name: String,
    pub torrent_file: TorrentFile,
    pub info_hash: Sha1Hash,
    pub pieces_left: Vec<usize>,
    pub dest_path: String,
}

impl Torrent {
    pub async fn new(torrent_name: String, dest_path: String) -> Torrent {
        let torrent_file = TorrentFile::new(&torrent_name).await;

        let info_hash = match TorrentFile::get_info_hash(torrent_file.get_bencoded_dict_ref()) {
            Some(info_hash) => info_hash,
            None => panic!("Could not get info hash from torrent file: {}", torrent_name)
        };

        // not downloaded piece indexes
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

            let pieces = match pieces {
                BencodedValue::ByteSha1Hashes(pieces) => pieces,
                _ => panic!("Could not get pieces from info dict ref in torrent file: {}", torrent_name)
            };

            let pieces_left = (0..pieces.len()).collect::<Vec<usize>>();

            pieces_left
        };

        Torrent {
            torrent_name,
            torrent_file,
            info_hash,
            pieces_left,
            dest_path
        }
    }

    pub fn get_info_hash_ref(&self) -> &Sha1Hash {
        &self.info_hash
    }

    pub fn get_dest_path(&self) -> &String {
        &self.dest_path
    }

    pub fn get_piece_length(&self, piece_index: usize) -> u64 {
        let torrent_dict = match self.torrent_file.get_bencoded_dict_ref().try_into_dict() {
            Some(torrent_dict) => torrent_dict,
            None => panic!("Could not get torrent dict ref from torrent file: {}", self.torrent_name)
        };
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {}", self.torrent_name)
        };

        let piece_length = match info_dict.get_from_dict("piece length") {
            Some(piece_length) => piece_length.try_into_integer().unwrap().clone() as u64,
            None => panic!("Could not get piece length from info dict ref in torrent file: {}", self.torrent_name)
        };

        if piece_index == self.get_pieces_count() - 1 {
            self.get_file_size() % piece_length
        }
        else {
            piece_length
        }
    }

    pub fn get_pieces_count(&self) -> usize {
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
            pieces.len()
        }
        else {
            panic!("Could not get pieces from info dict ref in torrent file: {}", self.torrent_name)
        }
    }

    pub fn get_file_size(&self) -> u64 {
        let torrent_dict = match self.torrent_file.get_bencoded_dict_ref().try_into_dict() {
            Some(torrent_dict) => torrent_dict,
            None => panic!("Could not get torrent dict ref from torrent file: {}", self.torrent_name)
        };
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {}", self.torrent_name)
        };

        let mut total_size: u64 = 0;

        if let Some(files_dict) = info_dict.get_from_dict("files") {
            let files = match files_dict {
                BencodedValue::List(files) => files,
                _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };

            for file in files {
                let file = match file {
                    BencodedValue::Dict(file) => file,
                    _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
                };

                let length = match file.get("length") {
                    Some(length) => length,
                    None => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
                };

                let length = match length {
                    BencodedValue::Integer(length) => length,
                    _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
                };

                total_size += *length as u64;
            }
        }
        else { 
            let file_length = match info_dict.get_from_dict("length") {
                Some(file_length) => file_length,
                None => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            }; 
            let file_length = match file_length {
                BencodedValue::Integer(file_length) => file_length,
                _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };

            total_size = file_length as u64;
        }
        
        total_size
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
        
        let mut all_files = Vec::new();

        if let Some(files_dict) = info_dict.get_from_dict("files") {
            let files = match files_dict {
                BencodedValue::List(files) => files,
                _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };

            for file in files {
                let file = match file {
                    BencodedValue::Dict(file) => file,
                    _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
                };

                all_files.push(file.clone());
            }
        }
        else {
            let file_name = match info_dict.get_from_dict("name") {
                Some(file_name) => file_name,
                None => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };
            let file_name = match file_name {
                BencodedValue::ByteString(file_name) => file_name,
                _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };

            let length = match info_dict.get_from_dict("length") {
                Some(length) => length,
                None => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };
            let length = match length {
                BencodedValue::Integer(length) => length,
                _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };

            let mut file_dict = BTreeMap::new();
            let path = BencodedValue::List(vec![BencodedValue::ByteString(file_name)]);
            file_dict.insert("path".to_string(), path);
            file_dict.insert("length".to_string(), BencodedValue::Integer(length.clone()));
            all_files.push(file_dict);
        }

        all_files
    }
}