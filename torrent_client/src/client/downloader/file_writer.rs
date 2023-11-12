use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};

pub mod torrent_writer;

use torrent_writer::{ TorrentWriter, TorrentWriterHandle };

#[derive(Debug)]
pub struct FileWriter {
    torrent_writers: Arc<Mutex<Vec<TorrentWriter>>>,
    downloader_rx: mpsc::Receiver<String>
}

impl FileWriter {
    pub async fn new(rx: mpsc::Receiver<String>) -> FileWriter {
        FileWriter { 
            torrent_writers: Arc::new(Mutex::new(Vec::new())),
            downloader_rx: rx
        }
    }

    pub async fn run(mut self, mut writer_rx: mpsc::Receiver<String>) {
        // create a torrent writer
        // run the torrent writer

        println!("Hello from FileWriter");

        let _ = tokio::join!(
            tokio::spawn(async move {
                // wait for torrent 
                while let Some(file_name) = self.downloader_rx.recv().await {
                    println!("File Writer got file_name: {file_name}");
                    // save file
                    let (torrent_writer, rx)  = TorrentWriter::new(file_name.clone()).await;

                    // self.push_to_torrent_writers(torrent_writer);

                    FileWriter::push_to_torrent_writers(self.torrent_writers.clone(), torrent_writer);

                    let torrent_writer_handle = TorrentWriterHandle::new(file_name.clone(), rx).await;

                    torrent_writer_handle.run().await;
                }
            }),
            tokio::spawn(async move {
                while let Some(torrent_piece) = writer_rx.recv().await {
                    println!("Got torrent_piece: {torrent_piece:?}");
                    
                    // let torrent_name = torrent_piece.get_torrent_name().await;
                    let torrent_name = torrent_piece;

                    let writer_tx = match FileWriter::get_torrent_tx(self.torrent_writers.clone(), torrent_name.clone()).await {
                        Some(writer_tx) => writer_tx,
                        None => panic!("no torrent tx for torrent: {torrent_name}")
                    };

                    // let piece = torrent_piece.get_piece().await;

                    writer_tx.send("hello".to_string()).await.unwrap();

                }
            }),
        );
    }

    async fn get_torrent_tx(torrent_writers: Arc<Mutex<Vec<TorrentWriter>>>, torrent_name: String) -> Option<mpsc::Sender<String>> { 
        let torrent_writers = torrent_writers.lock().await;

        for torrent_writer in torrent_writers.iter() {
            if torrent_writer.get_name().await == torrent_name {
                return Some(torrent_writer.get_torrent_tx().await);
            }
        }
        None
    }

    async fn push_to_torrent_writers(torrent_writers: Arc<Mutex<Vec<TorrentWriter>>>, torrent_writer: TorrentWriter) {
        let mut torrent_writers = torrent_writers.lock().await;

        torrent_writers.push(torrent_writer);
    }

}