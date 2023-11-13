use std::sync::Arc;

use tokio::{sync::{mpsc, Mutex}, fs::File};

#[derive(Debug)]
pub struct Peer {
    pub id: [u8;20]
}

impl Peer {
    pub async fn new() -> Peer {
        Peer {
            id: [0;20]
        }
    }

    pub async fn get_from_torrent(torrent: &Torrent) -> Vec<Peer> {
        vec![Peer::new().await, Peer::new().await]
    }
}

#[derive(Debug, Clone)]
pub struct Torrent {
    torrent_name: String
}

impl Torrent {
    // pub async fn new() -> Torrent {}

    pub async fn parse_from_file(torrent_name: String) -> Torrent {
        Torrent {
            torrent_name
        }
    }
}

pub struct PeerDownloader {
    pub peer_id: [u8;20],
    pub handler_rx: mpsc::Receiver<String>
}

impl PeerDownloader {
    pub async fn new(peer_id: [u8;20], handler_rx: mpsc::Receiver<String>) -> PeerDownloader {
        PeerDownloader {
            peer_id,
            handler_rx
        }
    } 
}

pub struct PeerDownloaderHandler {
    peer: Peer,
    torrent: Torrent,
    file: Arc<Mutex<File>>,
    downloader_tx: mpsc::Sender<String>,
}

impl PeerDownloaderHandler {

    pub async fn new(peer: Peer, torrent: Torrent, file: Arc<Mutex<File>>,downloader_tx: mpsc::Sender<String>) -> PeerDownloaderHandler {
        PeerDownloaderHandler {
            peer,
            torrent,
            file,
            downloader_tx
        }
    }

    pub async fn run(self) {
        println!("Hello from PeerDownloaderHandler");
        let _ = self.downloader_tx.send("Sending from PeerDownloader".to_string()).await.map_err(|e| {
            println!("Error sending from PeerDownloader: {}", e);
        });
    }
    
}