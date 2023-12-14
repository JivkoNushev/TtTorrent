use crate::utils::{read_file_as_bytes, sha1_hash};
use super::TorrentParser;

pub mod bencoded_value;
pub use bencoded_value::BencodedValue;

pub mod sha1hash;
pub use sha1hash::Sha1Hash;

#[derive(Debug, Clone)]
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
}

// TorrentFile methods
impl TorrentFile {
    pub async fn new(torrent_file_name: &str) -> std::io::Result<TorrentFile> {
        let torrent_file = read_file_as_bytes(torrent_file_name).await?;

        let bencoded_dict = TorrentParser::parse_torrent_file(&torrent_file);

        Ok(TorrentFile { bencoded_dict })
    }
}

