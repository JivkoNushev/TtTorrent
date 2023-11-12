use tokio::sync::mpsc;

pub mod peer_downloader;

use peer_downloader::{ PeerDownloader, Torrent, Peer };

pub struct TorrentDownloader {
    torrent_name: String,
}

impl TorrentDownloader {
    pub async fn new(torrent_name: String) -> TorrentDownloader {
        TorrentDownloader {
            torrent_name,
        }
    }
}

pub struct TorrentDownloaderHandler {
    torrent_name: String,
    peer_downloaders: Vec<PeerDownloader>,
    writer_tx: mpsc::Sender<String>,

}

impl TorrentDownloaderHandler {
    pub async fn new(torrent_name: String, writer_tx: mpsc::Sender<String>) -> TorrentDownloaderHandler {
        TorrentDownloaderHandler { 
            torrent_name: torrent_name,
            peer_downloaders: Vec::new(),
            writer_tx
        }
    }

    pub async fn run(self) {
        // parse torrent file
        let torrent = Torrent::parse_from_file(self.torrent_name).await;
        // get peers
        let peers = Peer::get_from_torrent(&torrent).await;

        // download from peers
        
        for peer in peers {
            let tx_clone = self.writer_tx.clone();
            let torrent_clone = torrent.clone();
            tokio::spawn(async move {
                let peer_downloader = PeerDownloader::new(torrent_clone, peer, tx_clone).await;

                peer_downloader.run().await;
            });
        }

    }
}
