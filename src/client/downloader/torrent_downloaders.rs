use tokio::sync::{mpsc, Mutex};
use lazy_static::lazy_static;
use tokio_stream::StreamExt;

use std::sync::Arc;
use std::collections::HashMap;

use crate::peer::Peer;
use crate::torrent::Torrent;
use crate::client::messager::{ InterProcessMessage, MessageType };

pub mod peer_downloader;
use peer_downloader::{ PeerDownloader, PeerDownloaderHandler};

lazy_static! {
    static ref PEER_DOWNLOADERS: Mutex<HashMap<String, Vec<PeerDownloader>>> = Mutex::new(HashMap::new());
}

pub struct TorrentDownloader {
    pub torrent: Arc<Mutex<Torrent>>,
    pub handler_rx: mpsc::Receiver<InterProcessMessage>,
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

    async fn peer_downloaders_remove(torrent_name: String, peer_id: [u8; 20]) {
        let mut guard = PEER_DOWNLOADERS.lock().await;

        if let Some(peer_downloaders) = guard.get_mut(&torrent_name) {
            peer_downloaders.retain(|peer_downloader| 
                peer_downloader.peer_id != peer_id
            );
        }
    }
}

// TorrentDownloader methods
impl TorrentDownloader {
    pub async fn new(torrent: Arc<Mutex<Torrent>>, handler_rx: mpsc::Receiver<InterProcessMessage>) -> std::io::Result<TorrentDownloader> {
        Ok(TorrentDownloader {
            torrent,
            handler_rx,
            peer_downloaders: Vec::new()
        })
    }
}

pub struct TorrentDownloaderHandler {
    torrent: Arc<Mutex<Torrent>>,
    downloader_tx: mpsc::Sender<InterProcessMessage>,
}

impl TorrentDownloaderHandler {
    pub fn new(torrent: Arc<Mutex<Torrent>>, downloader_tx: mpsc::Sender<InterProcessMessage>) -> TorrentDownloaderHandler {
        TorrentDownloaderHandler { 
            torrent,
            downloader_tx
        }
    }

    async fn get_new_peers(&self, peers: &mut Vec<Peer>) {
        let peer_ids = peers.iter()
            .map(|peer| 
                peer.id.clone()
            ).collect::<Vec<[u8; 20]>>();

        // stream of peers in peer_downloaders
        let guard = PEER_DOWNLOADERS.lock().await;
        let mut stream_of_peers = tokio_stream::iter(guard.iter());

        while let Some(v) = stream_of_peers.next().await {
            for peer_downloader in v.1.iter() {
                if peer_ids.contains(&peer_downloader.peer_id) {
                    peers.retain(|peer| 
                        peer.id != peer_downloader.peer_id
                    );
                }
            }
        }
    }

    async fn download_torrent(&mut self) -> std::io::Result<()> {
        loop {
            // if torrent is finished exit
            if self.torrent.lock().await.pieces_left.is_empty() {
                // send to the downloader that the torrent is finished

                let msg = InterProcessMessage::new(
                    MessageType::DownloaderFinished, 
                    self.torrent.lock().await.torrent_name.clone(),
                    Vec::new()
                );

                self.downloader_tx.send(msg).await.unwrap();

                return Ok(());
            }

            let mut peers = Peer::get_from_torrent(&self.torrent).await;

            // get new peers
            self.get_new_peers(&mut peers).await;

            if peers.is_empty() {
                // tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
                continue;
            }

            let mut stream_of_peers = tokio_stream::iter(peers);

            let torrent_name = self.torrent.lock().await.torrent_name.clone();

            while let Some(peer) = stream_of_peers.next().await {
                let (tx, rx) = mpsc::channel::<InterProcessMessage>(100);
                let peer_downloader = PeerDownloader::new(peer.id.clone(), rx);
                
                TorrentDownloader::peer_downloaders_push(torrent_name.clone(), peer_downloader).await;

                let tx_clone = tx.clone();
                let torrent_clone = Arc::clone(&self.torrent);
                let torrent_name_clone = torrent_name.clone();

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

                    TorrentDownloader::peer_downloaders_remove(torrent_name_clone, peer.id).await;
                });
            }
        }
    }


    async fn peer_downloader_recv_msg() -> Option<InterProcessMessage> {
        let mut guard = PEER_DOWNLOADERS.lock().await;

        if guard.is_empty() {
            return None;
        }

        let mut stream = tokio_stream::iter(guard.iter_mut());

        while let Some(v) = stream.next().await {
            for peer_downloader in v.1.iter_mut() {
                if let Some(msg) = &peer_downloader.handler_rx.recv().await {
                    return Some(msg.clone());
                }
            }
        }

        None
    }

    pub async fn run(mut self) {
        let downloader_tx_clone = self.downloader_tx.clone();

        let _ = tokio::join!(
            self.download_torrent(),
            tokio::spawn(async move {
                while let Some(msg) = TorrentDownloaderHandler::peer_downloader_recv_msg().await {
                    match msg.message_type {
                        MessageType::DownloadedPiecesCount => {
                            // send to downloader
                            if let Err(e) = downloader_tx_clone.send(msg).await {
                                println!("Error sending message to downloader: {}", e);
                            }
                        },
                        _ => {}
                    }
                }
            }),
        );
    }
}