use tokio::sync::mpsc;

pub mod torrent_downloader;

use torrent_downloader::{ TorrentDownloaderHandler, TorrentDownloader };

pub struct TorrentsReader {
    torrent_downloders: Vec<TorrentDownloader>,
    downloader_rx: mpsc::Receiver<String>
}

impl TorrentsReader {
    pub async fn new(rx: mpsc::Receiver<String>) -> TorrentsReader {
        TorrentsReader { 
            torrent_downloders: Vec::new(),
            downloader_rx: rx
        }
    }

    pub async fn run(mut self, writer_tx: mpsc::Sender<String>) {
        println!("hello from TorrentReader");

        let _ = tokio::join!(
            tokio::spawn(async move {
                // wait for torrent 
                while let Some(torrent_name) = self.downloader_rx.recv().await {
                    println!("Torrent Reader got torrent_name: {torrent_name}");
                    
                    // TODO: CREATE A TORRENT_DOWNLOADER HANDLE FOR THE ASYNC
                    // AND A TORRENT_DOWNLOADER FOR THE STRUCTURE THAT HOLDS THE 
                    // TORRENT NAME AND PIPES FOR COMUNICATION

                    // create a torrent downloader
                    let torrent_downloader_handler = TorrentDownloaderHandler::new(torrent_name.clone(), writer_tx.clone()).await;
                    // let torrent_downloader = TorrentDownloader::new(torrent_name.clone()).await;

                    // run the torrent downloader
                    torrent_downloader_handler.run().await;
                }
            }),
            // tokio::spawn(async move {
            //     // async download ??
            // }),
        );

    }
}
