use tokio::sync::mpsc;

pub mod torrent_writer;

use torrent_writer::TorrentWriter;

pub struct FileWriter {
    torrent_writers: Vec<TorrentWriter>,
    downloader_rx: mpsc::Receiver<String>
}

impl FileWriter {
    pub async fn new(rx: mpsc::Receiver<String>) -> FileWriter {
        FileWriter { 
            torrent_writers: Vec::new(),
            downloader_rx: rx
        }
    }

    pub async fn run(mut self) {
        // wait for torrent 
        // create a torrent writer
        // run the torrent writer

        println!("Hello from FileWriter");

        while let Some(command) = self.downloader_rx.recv().await {
            println!("File Writer got command: {command}");
        }
    }
}