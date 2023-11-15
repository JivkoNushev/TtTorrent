pub mod torrent_file;
pub mod torrent_parser;

pub use torrent_file::{TorrentFile, Sha1Hash};

pub use torrent_parser::TorrentParser;

#[derive(Debug, Clone)]
pub struct Torrent {
    pub torrent_name: String,
    pub torrent_file: TorrentFile,
    pub info_hash: Sha1Hash,
}

impl Torrent {
    pub async fn new(torrent_name: String) -> Torrent {
        let torrent_file = TorrentFile::new(torrent_name.clone()).await;

        let info_hash = TorrentFile::get_info_hash(torrent_file.get_bencoded_dict_ref())
            .await
            .unwrap();

        Torrent {
            torrent_name,
            torrent_file,
            info_hash
        }
    }

    pub async fn get_info_hash_ref(&self) -> &Sha1Hash {
        &self.info_hash
    }
}
