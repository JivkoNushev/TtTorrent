use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use lazy_static::lazy_static;

use std::sync::Arc;

use super::messager::{ InterProcessMessage, MessageType };

use torrent_downloaders::{ TorrentDownloader, TorrentDownloaderHandler };
pub mod torrent_downloaders;

lazy_static! {
    static ref TORRENT_DOWNLOADERS: Mutex<Vec<TorrentDownloader>> = Mutex::new(Vec::new());
}

pub struct Downloader {
    _client_tx: mpsc::Sender<InterProcessMessage>,
    client_rx: mpsc::Receiver<InterProcessMessage>,
}


// TORRENT_DOWNLOADERS methods
impl Downloader {
    async fn torrent_downloaders_push(torrent_downloader: TorrentDownloader) {
        TORRENT_DOWNLOADERS.lock().await.push(torrent_downloader);
    }
}

// Downloader methods
impl Downloader {
    pub fn new(tx: mpsc::Sender<InterProcessMessage>, rx: mpsc::Receiver<InterProcessMessage>) -> Downloader {
        Downloader {
            _client_tx: tx,
            client_rx: rx,
        }
    }
    
    async fn download_torrent(torrent_name: String, dest_path: String) {
        let (tx, rx) = mpsc::channel::<InterProcessMessage>(100);

        let torrent_downloader = TorrentDownloader::new(torrent_name.clone(), dest_path, rx).await;
    
        let torrent = Arc::clone(&torrent_downloader.torrent);

        Downloader::torrent_downloaders_push(torrent_downloader).await;

        println!("Downloading torrent file: {}", torrent_name);
        tokio::spawn(async move {
            let torrent_downloader_handler = TorrentDownloaderHandler::new(torrent, tx);
            torrent_downloader_handler.run().await;
        });
    }

    async fn torrent_downloader_recv_msg() -> Option<InterProcessMessage> {
        let mut guard = TORRENT_DOWNLOADERS.lock().await;

        let mut stream = tokio_stream::iter(guard.iter_mut());

        while let Some(v) = stream.next().await {
            if let Some(msg) = &v.handler_rx.recv().await {
                return Some(msg.clone());
            }
        }

        None
    }

    pub async fn run(mut self) {
        let _ = tokio::join!(
            // Receive messages from client
            tokio::spawn(async move {
                loop {
                    if let Some(message) = self.client_rx.recv().await {
                        match message.message_type {
                            MessageType::DownloadTorrent => {
                                let dest_path = String::from_utf8(message.payload).unwrap();
                                Downloader::download_torrent(message.torrent_name, dest_path).await;
                            },
                            _ => todo!("Received an unknown message from the client: {:?}", message)
                        }
                    }
                }
            }),
            // Receive messages from torrent downloders 
            tokio::spawn(async move {
                loop {
                    if let Some(msg) = Downloader::torrent_downloader_recv_msg().await {
                        match msg.message_type {
                            MessageType::DownloadedPiecesCount => {
                                let _ = self._client_tx.send(msg).await;
                            },
                            _ => todo!("Received an unknown message from the torrent downloader: {:?}", msg)
                        }
                    }
                }
            }),
        );
        
    }
}
