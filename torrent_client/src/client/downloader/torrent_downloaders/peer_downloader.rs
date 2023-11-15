use std::sync::Arc;

use tokio::{sync::{mpsc, Mutex}, fs::File};

use crate::torrent::Torrent;
use crate::peer::Peer;

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
        let msg = format!("Hello from PeerDownloaderHandler: {:?}", self.peer.id);

        let _ = self.downloader_tx.send(msg).await.map_err(|e| {
            println!("Error sending from PeerDownloader: {}", e);
        });

        println!("Torrent info hash: {:?}", self.torrent.info_hash);

        println!("Peer id: {:?}", self.peer.id);

        // send handshake

        // receive handshake

        // wait for bitmap

        // send interested

        // receive unchoke
    }
    
}