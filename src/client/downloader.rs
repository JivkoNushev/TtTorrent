use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use lazy_static::lazy_static;

use std::sync::Arc;

use crate::torrent::Torrent;

use super::messager::{ InterProcessMessage, MessageType };

use torrent_downloaders::{ TorrentDownloader, TorrentDownloaderHandler };
pub mod torrent_downloaders;

lazy_static! {
    static ref TORRENT_DOWNLOADERS: Mutex<Vec<TorrentDownloader>> = Mutex::new(Vec::new());
}

pub struct Downloader {
    client_tx: mpsc::Sender<InterProcessMessage>,
    client_rx: mpsc::Receiver<InterProcessMessage>,
}


// TORRENT_DOWNLOADERS methods
impl Downloader {
    async fn torrent_downloaders_push(torrent_downloader: TorrentDownloader) {
        TORRENT_DOWNLOADERS.lock().await.push(torrent_downloader);
    }

    async fn torrent_downloaders_remove(torrent_name: String) {
        let mut downloaders = TORRENT_DOWNLOADERS.lock().await;
        
        for (i, downloader) in downloaders.iter().enumerate() {
            let torrent_name_ = downloader.torrent.lock().await.torrent_name.clone();

            if torrent_name == torrent_name_ {
                downloaders.remove(i);
                break;
            }
        }
    }
}

// Downloader methods
impl Downloader {
    pub fn new(tx: mpsc::Sender<InterProcessMessage>, rx: mpsc::Receiver<InterProcessMessage>) -> Downloader {
        Downloader {
            client_tx: tx,
            client_rx: rx,
        }
    }

    async fn save_state() {
        // if state file doesn't exist create it
        if !tokio::fs::metadata("client_state.json").await.is_ok() {
            let _ = tokio::fs::File::create("client_state.json").await;
        }

        let state_as_str = match tokio::fs::read_to_string("client_state.json").await {
            Ok(state) => state,
            Err(e) => {
                println!("Failed to read state file: {}", e);
                return;
            }
        };

        let mut state = serde_json::json!({});
        if !state_as_str.is_empty() {
            state = match serde_json::from_str::<serde_json::Value>(&state_as_str) {
                Ok(state) => state,
                Err(e) => {
                    println!("Failed to parse state file: {}", e);
                    return;
                }
            }; 
        }

        state["downloading"] = serde_json::json!([]);

        // save the state of the downloader
        for torrent_downloader in TORRENT_DOWNLOADERS.lock().await.iter() {
            let torrent = torrent_downloader.torrent.lock().await;
            
            if torrent.pieces_left.is_empty() {
                if !state["downloaded"].is_array() {
                    state["downloaded"] = serde_json::json!([]);
                }

                state["downloaded"].as_array_mut().unwrap().push(serde_json::json!(*torrent));
            }
            else {
                state["downloading"].as_array_mut().unwrap().push(serde_json::json!(*torrent));
            }
        }

        // save to file
        let state_as_str = serde_json::to_string_pretty(&state).unwrap();
        if let Err(e) = tokio::fs::write("client_state.json", state_as_str).await {
            println!("Failed to write state file: {}", e);
        }
    }

    async fn load_state(&mut self) {
        if !tokio::fs::metadata("client_state.json").await.is_ok() {
            return;
        }

        let state = match tokio::fs::read_to_string("client_state.json").await {
            Ok(state) => state,
            Err(e) => {
                println!("Failed to read state file: {}", e);
                return;
            }
        };

        if state.is_empty() {
            return;
        }

        let state = match serde_json::from_str::<serde_json::Value>(&state) {
            Ok(state) => state,
            Err(e) => {
                println!("Failed to parse state file: {}", e);
                return;
            }
        };

        // if state doesn't have "downloading" key return
        if !state["downloading"].is_array() {
            return;
        }

        state["downloading"].as_array().unwrap().iter().for_each(|torrent| {
            let torrent = serde_json::from_value::<Torrent>(torrent.clone()).unwrap();

            // download torrent
            let _ = tokio::spawn(async move {
                if let Err(e) = Downloader::download_torrent(Arc::new(Mutex::new(torrent))).await {
                    println!("Failed to download torrent: {}", e);
                }
            });
        });
    }
    
    async fn download_torrent(torrent: Arc<Mutex<Torrent>>) -> std::io::Result<()> {
        let (tx, rx) = mpsc::channel::<InterProcessMessage>(100);

        let torrent_name = torrent.lock().await.torrent_name.clone();

        let torrent_downloader = TorrentDownloader::new(Arc::clone(&torrent), rx).await?;
    
        Downloader::torrent_downloaders_push(torrent_downloader).await;

        println!("Downloading torrent file: {}", torrent_name);
        tokio::spawn(async move {
            let torrent_downloader_handler = TorrentDownloaderHandler::new(torrent, tx);
            torrent_downloader_handler.run().await;

        });

        Ok(())
    }

    async fn torrent_downloader_recv_msg() -> Option<InterProcessMessage> {
        let mut guard = TORRENT_DOWNLOADERS.lock().await;

        if guard.is_empty() {
            return None;
        }

        let mut stream = tokio_stream::iter(guard.iter_mut());

        while let Some(v) = stream.next().await {
            if let Some(msg) = &v.handler_rx.recv().await {
                return Some(msg.clone());
            }
        }

        None
    }

    pub async fn run(mut self) {
        // get last state of the downloader
        self.load_state().await;

        let client_tx_clone = self.client_tx.clone();   

        let _ = tokio::join!(
            // Receive messages from client
            tokio::spawn(async move {
                loop {
                    if let Some(message) = self.client_rx.recv().await {
                        match message.message_type {
                            MessageType::DownloadTorrent => {
                                let dest_path = String::from_utf8(message.payload).unwrap();

                                let torrent = match Torrent::new(message.torrent_name, dest_path).await {
                                    Ok(torrent) => Arc::new(Mutex::new(torrent)),
                                    Err(e) => {
                                        println!("Failed to create torrent: {}", e);
                                        continue;
                                    }
                                };

                                if let Err(e) = Downloader::download_torrent(torrent).await {
                                    println!("Failed to download torrent: {}", e);
                                }
                            },
                            MessageType::SaveState => {
                                Downloader::save_state().await;

                                // send a message to the client that the downloader finished
                                let finished_message = InterProcessMessage::new(
                                    MessageType::DownloaderFinished,
                                    String::new(),
                                    Vec::new()
                                );

                                let _ = client_tx_clone.send(finished_message).await;
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
                                let _ = self.client_tx.send(msg).await;
                            },
                            MessageType::DownloaderFinished => {
                                let torrent_name = msg.torrent_name.clone();
                                Downloader::save_state().await;
                                Downloader::torrent_downloaders_remove(torrent_name.clone()).await;
                            },
                            _ => todo!("Received an unknown message from the torrent downloader: {:?}", msg)
                        }
                    }
                }
            }),
        );
        
    }
}
