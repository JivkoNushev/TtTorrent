pub mod peer_downloader;

use peer_downloader::PeerDownloader;

pub struct TorrentDownloader {
    torrent_name: String,
    peer_downloaders: Vec<PeerDownloader>,
}

impl TorrentDownloader {
    pub async fn new(torrent_name: String) -> TorrentDownloader {
        TorrentDownloader { 
            torrent_name: torrent_name,
            peer_downloaders: Vec::new(),
        }
    }

    pub async fn run(self) {
        // parse torrent file
        // get peers
        // download from peers
    }
}
