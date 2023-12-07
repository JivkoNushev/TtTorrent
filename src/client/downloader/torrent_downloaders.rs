use tokio::sync::{mpsc, Mutex};
use lazy_static::lazy_static;
use tokio_stream::StreamExt;

use std::sync::Arc;
use std::collections::HashMap;

use crate::peer::Peer;
use crate::torrent::Torrent;

pub mod peer_downloader;
use peer_downloader::{ PeerDownloader, PeerDownloaderHandler};

lazy_static! {
    static ref PEER_DOWNLOADERS: Mutex<HashMap<String, Vec<PeerDownloader>>> = Mutex::new(HashMap::new());
}

pub struct TorrentDownloader {
    pub torrent: Arc<Mutex<Torrent>>,
    pub handler_rx: mpsc::Receiver<String>,
    pub peer_downloaders: Vec<PeerDownloader>,
}

// PEER_DOWNLOADERS methods
impl TorrentDownloader {
    async fn peer_downloaders_push(torrent_name: String, peer_downloader: PeerDownloader) {
        let mut guard = PEER_DOWNLOADERS.lock().await;

        if let Some(peer_downloaders) = guard.get_mut(&torrent_name) {
            peer_downloaders.push(peer_downloader);
        } else {
            guard.insert(torrent_name, vec![peer_downloader]);
        }
    }
}

// TorrentDownloader methods
impl TorrentDownloader {
    pub async fn new(torrent_name: String, dest_path: String, handler_rx: mpsc::Receiver<String>) -> TorrentDownloader {
        let torrent = Torrent::new(torrent_name, dest_path).await;
        let torrent = Arc::new(Mutex::new(torrent));
        
        TorrentDownloader {
            torrent,
            handler_rx,
            peer_downloaders: Vec::new()
        }
    }

    pub async fn get_downloaded_percentage(&self) -> f32 {
        let torrent = self.torrent.lock().await;

        let pieces_left = torrent.pieces_left.len();
        let total_pieces = torrent.get_pieces_count();

        let downloaded_percentage = (total_pieces - pieces_left) as f32 / total_pieces as f32;
        let downloaded_percentage = (downloaded_percentage * 100.0).round() / 100.0;

        downloaded_percentage
    }
}

pub struct TorrentDownloaderHandler {
    torrent: Arc<Mutex<Torrent>>,
    _downloader_tx: mpsc::Sender<String>,
}

impl TorrentDownloaderHandler {
    pub fn new(torrent: Arc<Mutex<Torrent>>, _downloader_tx: mpsc::Sender<String>) -> TorrentDownloaderHandler {
        TorrentDownloaderHandler { 
            torrent,
            _downloader_tx
        }
    }

    async fn download_torrent(&mut self) -> std::io::Result<()> {
        let peers = Peer::get_from_torrent(&self.torrent).await;

        if peers.len() == 0 {
            return std::io::Result::Err(std::io::Error::new(std::io::ErrorKind::Other, "No peers found"));
        }

        let mut stream_of_peers = tokio_stream::iter(peers);

        let torrent_name = self.torrent.lock().await.torrent_name.clone();

        while let Some(peer) = stream_of_peers.next().await {
            let (tx, rx) = mpsc::channel::<String>(100);
            let peer_downloader = PeerDownloader::new(peer.id.clone(), rx);
            
            TorrentDownloader::peer_downloaders_push(torrent_name.clone(), peer_downloader).await;

            let tx_clone = tx.clone();
            let torrent_clone = Arc::clone(&self.torrent);

            tokio::spawn(async move {
                let peer_downloader_handle = PeerDownloaderHandler::new(peer.clone(), torrent_clone, tx_clone);

                match peer_downloader_handle.run().await {
                    Ok(_) => {
                        println!("Finished downloading from peer '{}'", peer);
                    },
                    Err(e) => {
                        println!("Peer '{}' finished with error: {}", peer, e);
                    }
                }
            });
        }

        Ok(())
    }


    async fn peer_downloader_recv_msg() -> Option<(String, String)> {
        let mut guard = PEER_DOWNLOADERS.lock().await;

        let mut stream = tokio_stream::iter(guard.iter_mut());

        while let Some(v) = stream.next().await {
            for peer_downloader in v.1.iter_mut() {
                if let Some(msg) = &peer_downloader.handler_rx.recv().await {
                    return Some((v.0.clone(), msg.clone()));
                }
            }
        }

        None
    }

    pub async fn run(mut self) {
        let _ = tokio::join!(
            self.download_torrent(),

            tokio::spawn(async move {
                loop {
                    if let Some((torrent_name, msg)) = TorrentDownloaderHandler::peer_downloader_recv_msg().await {
                        println!("For torrent file '{torrent_name}' Received peer message: {msg}");
                    }
                }
            }),
        );
    }
}