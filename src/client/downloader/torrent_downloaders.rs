use tokio::sync::{mpsc, Mutex};
use lazy_static::lazy_static;
use tokio_stream::StreamExt;

use std::{sync::Arc, collections::HashMap};

pub mod peer_downloader;

use peer_downloader::{ PeerDownloader, PeerDownloaderHandler};

use crate::peer::Peer;
use crate::torrent::Torrent;

lazy_static! {
    static ref PEER_DOWNLOADERS: Mutex<HashMap<String, Vec<PeerDownloader>>> = Mutex::new(HashMap::new());
}

pub struct TorrentDownloader {
    pub torrent_name: String,
    pub handler_rx: mpsc::Receiver<String>,
    pub peer_downloaders: Vec<PeerDownloader>,
}

impl TorrentDownloader {
    pub fn new(torrent_name: String, handler_rx: mpsc::Receiver<String>) -> TorrentDownloader {
        TorrentDownloader {
            torrent_name,
            handler_rx,
            peer_downloaders: Vec::new()
        }
    }
}

pub struct TorrentDownloaderHandler {
    torrent_name: String,
    downloader_tx: mpsc::Sender<String>,
}

impl TorrentDownloaderHandler {
    pub fn new(torrent_name: String, downloader_tx: mpsc::Sender<String>) -> TorrentDownloaderHandler {
        println!("Creating a TorrentDownloaderHandler for torrent file: {}", torrent_name);
        
        TorrentDownloaderHandler { 
            torrent_name,
            downloader_tx
        }
    }

    async fn peer_downloaders_push(torrent_name: String, peer_downloader: PeerDownloader) {
        let mut guard = PEER_DOWNLOADERS.lock().await;

        if let Some(peer_downloaders) = guard.get_mut(&torrent_name) {
            peer_downloaders.push(peer_downloader);
        } else {
            guard.insert(torrent_name, vec![peer_downloader]);
        }
    }

    async fn download_torrent(&mut self) {
        // parse torrent file
        println!("Parsing torrent file: {}", self.torrent_name);
        let torrent = Torrent::new(self.torrent_name.clone());

        // get peers
        println!("Getting peers for torrent file: {}", self.torrent_name);
        let peers = Peer::get_from_torrent(&torrent).await;

        // create a file
        println!("Creating a file for: {}", self.torrent_name);

        // TODO: Create a File type that has destination
        let file = tokio::fs::File::create("test.txt").await.unwrap();
        let file = Arc::new(Mutex::new(file));

        // download from peers
        let mut stream = tokio_stream::iter(peers);

        while let Some(peer) = stream.next().await {
            let (tx, rx) = mpsc::channel::<String>(100);
            let peer_downloader = PeerDownloader::new(peer.id.clone(), rx);
            
            TorrentDownloaderHandler::peer_downloaders_push(self.torrent_name.clone(), peer_downloader).await;

            let tx_clone = tx.clone();
            let torrent_clone = torrent.clone();
            let file_copy = Arc::clone(&file);

            tokio::spawn(async move {
                let peer_downloader_handle = PeerDownloaderHandler::new(peer, torrent_clone, file_copy, tx_clone);

                peer_downloader_handle.run().await;
            });
        }

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
            // downloading the torrent file
            self.download_torrent(),
            // Receiving messages from the peer downloaders
            tokio::spawn(async move {
                println!("Waiting for peer downloader messages...");
                loop {
                    if let Some((torrent_name, msg)) = TorrentDownloaderHandler::peer_downloader_recv_msg().await {
                        println!("For torrent file '{torrent_name}' Received peer message: {msg}");
                    }
                }
            }),
        );
    }
}