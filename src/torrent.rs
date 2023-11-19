pub mod torrent_file;
pub mod torrent_parser;

pub use torrent_file::{TorrentFile, Sha1Hash};
pub use torrent_parser::TorrentParser;
pub use self::torrent_file::BencodedValue;

#[derive(Debug, Clone)]
pub struct Torrent {
    pub torrent_name: String,
    pub torrent_file: TorrentFile,
    pub info_hash: Sha1Hash,
}

impl Torrent {
    pub fn new(torrent_name: String) -> Torrent {
        let torrent_file = TorrentFile::new(torrent_name.clone());

        let info_hash = match TorrentFile::get_info_hash(torrent_file.get_bencoded_dict_ref()) {
            Some(info_hash) => info_hash,
            None => panic!("Could not get info hash from torrent file: {}", torrent_name)
        };

        Torrent {
            torrent_name,
            torrent_file,
            info_hash
        }
    }

    pub fn get_info_hash_ref(&self) -> &Sha1Hash {
        &self.info_hash
    }

    pub fn get_piece_length(&self) -> i128 {
        let torrent_dict = match self.torrent_file.get_bencoded_dict_ref().try_into_dict() {
            Some(torrent_dict) => torrent_dict,
            None => panic!("Could not get torrent dict ref from torrent file: {}", self.torrent_name)
        };
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {}", self.torrent_name)
        };

        match info_dict.get_from_dict("piece length") {
            Some(piece_length) => piece_length.try_into_integer().unwrap().clone(),
            None => panic!("Could not get piece length from info dict ref in torrent file: {}", self.torrent_name)
        }
    }
}
