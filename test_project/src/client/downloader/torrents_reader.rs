use tokio::sync::mpsc;

pub mod torrent_downloader;

use torrent_downloader::TorrentDownloader;

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

    pub async fn run(mut self) {
        println!("hello from TorrentReader");

        let _ = tokio::join!(
            tokio::spawn(async move {
                // wait for torrent 
                while let Some(command) = self.downloader_rx.recv().await {
                    println!("Torrent Reader got command: {command}");
                    
                    // TODO: CREATE A TORRENT_DOWNLOADER HANDLE FOR THE ASYNC
                    // AND A TORRENT_DOWNLOADER FOR THE STRUCTURE THAT HOLDS THE 
                    // TORRENT NAME AND PIPES FOR COMUNICATION

                    // create a torrent downloader
                    let torrent_downloader = TorrentDownloader::new(command).await;

                    // run the torrent downloader
                    torrent_downloader.run().await;
                }
            }),
            // tokio::spawn(async move {
            //     // async download ??
            // }),
        );

    }
}
