use std::io::Write;

use interprocess::local_socket::LocalSocketStream;
use tokio::task::JoinHandle;
use tokio::sync::{oneshot, mpsc};

use anyhow::{Result, Context};

use crate::torrent::TorrentHandle;
use crate::messager::ClientMessage;

struct Client {
    rx: mpsc::Receiver<ClientMessage>,
    torrent_handles: Vec<TorrentHandle>,

    pub client_id: [u8; 20],
}

impl Client {
    pub fn new(client_id: [u8; 20], receiver: mpsc::Receiver<ClientMessage>) -> Self {
        Self {
            rx: receiver,
            torrent_handles: Vec::new(),
            client_id,
        }
    }

    async fn save_state(&self) -> Result<()> {
    
        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::DownloadTorrent{src, dst} => {
                            println!("Creating a new torrent handle for {} -> {}", src, dst);
                            let torrent_handle = TorrentHandle::new(self.client_id, &src, &dst).await?;
                            self.torrent_handles.push(torrent_handle);
                        },
                        ClientMessage::Shutdown => {
                            println!("Client stopping");
                            for torrent_handle in &mut self.torrent_handles {
                                let _ = torrent_handle.tx.send(ClientMessage::Shutdown).await;
                            }

                            break;
                        },
                        _ => {}
                    }
                }
            }
        }

        for torrent_handle in self.torrent_handles {
            let _ = torrent_handle.join_handle.await;
        }

        Ok(())
    }
}

pub struct ClientHandle {
    tx: mpsc::Sender<ClientMessage>,

    pub join_handle: JoinHandle<()>,
    pub id: [u8; 20],
}

impl ClientHandle {
    fn setup_graceful_shutdown(tx: mpsc::Sender<ClientMessage>) {
        // TODO: is this the best way to handle this?; add more signals

        // Spawn an async task to handle the signals
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    // Handle Ctrl+C (SIGINT)
                    println!("Ctrl+C received. Cleaning up...");
                },
            }

            let mut client_socket = match LocalSocketStream::connect("/tmp/TtTClient.sock") {
                Ok(socket) => socket,
                Err(_) => {
                    println!("Failed to connect to the client");
                    return;
                }
            };

            let message = ClientMessage::Shutdown;

            let serialized_data = serde_json::to_string(&message).expect("Serialization failed");

            client_socket.write_all(serialized_data.as_bytes()).expect("Failed to send data");
        });
    }

    pub fn new() -> Self {
        let id = "TtT-1-0-0-TESTCLIENT".as_bytes().try_into().unwrap();

        let (sender, receiver) = mpsc::channel(100);
        let client = Client::new(id, receiver);

        let join_handle = tokio::spawn(async move {
            if let Err(e) = client.run().await {
                println!("Client error: {:?}", e);
            }
        });

        Self::setup_graceful_shutdown(sender.clone());

        Self {
            tx: sender,
            join_handle,
            id,
        }
    }

    pub async fn client_download_torrent(&mut self, src: String, dst: String) -> Result<()> {
        let msg = ClientMessage::DownloadTorrent{src, dst};
        self.tx.send(msg).await.context("couldn't send a download message to the client")?;

        Ok(())
    }

    pub async fn client_shutdown(&mut self) -> Result<()> {
        let msg = ClientMessage::Shutdown;
        self.tx.send(msg).await.context("couldn't send a shutdown message to the client")?;

        Ok(())
    }
}
    
    
    