use tokio::sync::mpsc;

pub mod file_writer;
pub mod torrents_reader;

use file_writer::FileWriter;
use torrents_reader::TorrentsReader;

pub struct Downloader {
    file_writer: FileWriter,
    torrents_reader: TorrentsReader,
    client_rx: mpsc::Receiver<String>,
    file_writer_tx: mpsc::Sender<String>,
    torrents_reader_tx: mpsc::Sender<String>,
}

impl Downloader {
    pub async fn new(rx: mpsc::Receiver<String>) -> Downloader {
        let (file_writer_tx, file_writer_rx) = mpsc::channel::<String>(100);
        let (torrents_reader_tx, torrents_reader_rx) = mpsc::channel::<String>(100);
        
        Downloader {
            file_writer: FileWriter::new(file_writer_rx).await,
            torrents_reader: TorrentsReader::new(torrents_reader_rx).await,
            client_rx: rx,
            file_writer_tx: file_writer_tx,
            torrents_reader_tx: torrents_reader_tx
        }
    }

    pub async fn command_send_all(tx1: mpsc::Sender<String>, tx2: mpsc::Sender<String>, command: String) {
        let _ = tx1.send(command.clone()).await;
        let _ = tx2.send(command.clone()).await;
    }

    pub async fn run(mut self) {
        println!("Hello from downloader");

        let file_writer_handle = self.file_writer;
        let torrents_reader_handle = self.torrents_reader;

        let (rw_tx, rw_rx) = mpsc::channel::<String>(100);

        let _ = tokio::join!(
            torrents_reader_handle.run(rw_tx),
            file_writer_handle.run(rw_rx),
            tokio::spawn(async move {
                // println!("Waiting for a command...");
                while let Some(command) = self.client_rx.recv().await {
                    // println!("Sending the command to all");
                    Downloader::command_send_all(self.file_writer_tx.clone(), self.torrents_reader_tx.clone(), command).await;
                }
            }),
        );
    }
}
