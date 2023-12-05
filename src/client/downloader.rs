use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use lazy_static::lazy_static;

use crate::torrent::Torrent;

pub mod torrent_downloaders;

use tokio_stream::StreamExt;
use torrent_downloaders::{ TorrentDownloader, TorrentDownloaderHandler };

lazy_static! {
    static ref TORRENT_DOWNLOADERS: Mutex<Vec<TorrentDownloader>> = Mutex::new(Vec::new());
}

pub struct Downloader {
    client_tx: mpsc::Sender<String>,
    client_rx: mpsc::Receiver<String>,
}

impl Downloader {
    pub fn new(tx: mpsc::Sender<String>, rx: mpsc::Receiver<String>) -> Downloader {
        Downloader {
            client_tx: tx,
            client_rx: rx,
        }
    }

    async fn torrent_downloaders_push(torrent_downloader: TorrentDownloader) {
        TORRENT_DOWNLOADERS.lock().await.push(torrent_downloader);
    }
    
    async fn download_torrent(torrent_name: String) {
        let (tx, rx) = mpsc::channel::<String>(100);

        let torrent_downloader = TorrentDownloader::new(torrent_name.clone(), rx).await;
    
        let torrent = Arc::clone(&torrent_downloader.torrent);

        Downloader::torrent_downloaders_push(torrent_downloader).await;

        println!("Downloading torrent file: {}", torrent_name);
        tokio::spawn(async move {
            let torrent_downloader_handler = TorrentDownloaderHandler::new(torrent, tx);
            torrent_downloader_handler.run().await;
        });
    }

    async fn torrent_downloader_recv_msg() -> Option<String> {
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
            // Downloading given torrent files
            tokio::spawn(async move {
                loop {
                    if let Some(torrent_file) = self.client_rx.recv().await {
                        tokio::spawn(async move {
                            println!("Received a torrent file: {}", torrent_file);
                            Downloader::download_torrent(torrent_file).await;
                        }); 
                    }
                }
            }),
            // receiving messages from the torrent downloder 
            tokio::spawn(async move {
                loop {
                    if let Some(msg) = Downloader::torrent_downloader_recv_msg().await {
                        println!("Received torrent downloader message: {}", msg);
                        // self.client_tx.send(msg).await.unwrap();
                    }
                }
            }),
            // printing downloaded percentage for every torrent file
            tokio::spawn(async move {
                loop {
                    let guard = TORRENT_DOWNLOADERS.lock().await;

                    if guard.len() == 0 {
                        continue;
                    }

                    for torrent_downloader in guard.iter() {
                        let downloaded_percentage = torrent_downloader.get_downloaded_percentage().await;

                        let torrent_name = torrent_downloader.torrent.lock().await.torrent_name.clone();

                        println!("{torrent_name}: {downloaded_percentage}%");
                    }
                }
            })
        );
        
    }
}
