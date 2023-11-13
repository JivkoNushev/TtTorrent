use tokio::sync::{mpsc, Mutex};
use lazy_static::lazy_static;

pub mod torrent_downloaders;

use torrent_downloaders::{ TorrentDownloader, TorrentDownloaderHandler };

lazy_static! {
    static ref TORRENT_DOWNLOADERS: Mutex<Vec<TorrentDownloader>> = Mutex::new(Vec::new());
}

pub struct Downloader {
    client_tx: mpsc::Sender<String>,
    client_rx: mpsc::Receiver<String>,
}

impl Downloader {
    pub async fn new(tx: mpsc::Sender<String>, rx: mpsc::Receiver<String>) -> Downloader {
        
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
    
        Downloader::torrent_downloaders_push(torrent_downloader).await;

        println!("Downloading torrent file: {}", torrent_name);
        tokio::spawn(async move {
            let torrent_downloader_handler = TorrentDownloaderHandler::new(torrent_name, tx).await;

            torrent_downloader_handler.run().await;
        });
    }

    async fn torrent_downloader_recv_msg() -> Option<String> {
        let mut guard = TORRENT_DOWNLOADERS.lock().await;

        for torrent_downloader in guard.iter_mut() {
            if let Some(msg) = &torrent_downloader.handler_rx.recv().await {
                return Some(msg.clone());
            }
        }

        None
    }

    pub async fn run(mut self) {
        println!("Hello from downloader");

        let _ = tokio::join!(
            tokio::spawn(async move {
                println!("Waiting for a torrent file...");
                loop {
                    if let Some(torrent_file) = (&mut self).client_rx.recv().await {
                        tokio::spawn(async move {
                            println!("Received a torrent file: {}", torrent_file);
                            Downloader::download_torrent(torrent_file).await;
                        }); 
                    }
                }
            }),
            tokio::spawn(async move {
                println!("Waiting for a torrent file data...");
                loop {
                    if let Some(msg) = Downloader::torrent_downloader_recv_msg().await {
                        println!("Received torrent file data: {}", msg);
                    }
                }
            }),
        );
        
    }


}
