use interprocess::local_socket::LocalSocketStream;
use tokio::task::JoinHandle;
use tokio::sync::mpsc;
use anyhow::{Result, Context};

use std::io::Write;

use crate::peer::ConnectionType;
use crate::torrent::{TorrentHandle, TorrentState};
use crate::messager::ClientMessage;

struct Client {
    rx: mpsc::Receiver<ClientMessage>,
    torrent_handles: Vec<TorrentHandle>,

    client_id: [u8; 20],
}

impl Client {
    pub fn new(client_id: [u8; 20], receiver: mpsc::Receiver<ClientMessage>) -> Self {
        Self {
            rx: receiver,
            torrent_handles: Vec::new(),
            client_id,
        }
    }

    async fn load_state(&mut self) -> Result<()> {
        let client_state = match tokio::fs::read_to_string(crate::STATE_FILE_PATH).await {
            Ok(state) => state,
            Err(_) => {
                println!("No client state found");
                return Ok(());
            }
        };
        
        let client_state = match serde_json::from_str::<serde_json::Value>(&client_state) {
            Ok(state) => state,
            Err(e) => {
                println!("Failed to deserialize client state: {}", e);
                return Ok(());
            }
        };
        println!("Loading client state");

        if let Some(downloading) = client_state.get("Downloading") {
            for (key, val) in downloading.as_object().unwrap() {
                let torrent_state = match serde_json::from_value::<TorrentState>(val.clone()) {
                    Ok(torrent_state) => torrent_state,
                    Err(e) => {
                        println!("Failed to deserialize torrent state: {}", e);
                        continue;
                    }
                };

                println!("Loading torrent: {}", key);

                let torrent_handle = TorrentHandle::from_state(self.client_id, torrent_state, ConnectionType::Outgoing).await?;
                self.torrent_handles.push(torrent_handle);
            }
        }
        else {
            println!("No Downloading key found in client state");
        }

        // TODO: seeding
        if let Some(seeding) = client_state.get("Seeding") {
            for (key, val) in seeding.as_object().unwrap() {
                let torrent_context = match serde_json::from_value::<TorrentState>(val.clone()) {
                    Ok(torrent_context) => torrent_context,
                    Err(e) => {
                        println!("Failed to deserialize torrent state: {}", e);
                        continue;
                    }
                };

                println!("Loading torrent: {}", key);

                let torrent_handle = TorrentHandle::from_state(self.client_id, torrent_context, ConnectionType::Incoming).await?;
                self.torrent_handles.push(torrent_handle);
            }
        }
        else {
            println!("No Seeding key found in client state");
        }

        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
        // load the client state
        self.load_state().await?;

        let mut sending_to_terminal_client = false;

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
                                torrent_handle.shutdown().await?;
                            }

                            break;
                        },
                        ClientMessage::SendTorrentInfo => {
                            sending_to_terminal_client = true;
                        },
                        ClientMessage::TerminalClientClosed => {
                            sending_to_terminal_client = false;
                        },
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    if !sending_to_terminal_client {
                        continue;
                    }

                    // save state to file
                }
            }
        }

        for torrent_handle in self.torrent_handles {
            let _ = torrent_handle.join().await;
        }

        Ok(())
    }
}

pub struct ClientHandle {
    tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<()>,

    id: [u8; 20],
}

// static function implementations
impl ClientHandle {
    fn setup_graceful_shutdown() {
        // TODO: is this the best way to handle this?; add more signals

        // Spawn an async task to handle the signals
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    // Handle Ctrl+C (SIGINT)
                    println!("Ctrl+C received. Cleaning up...");
                },
            }

            let mut client_socket = match LocalSocketStream::connect(crate::SOCKET_PATH) {
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
}

impl ClientHandle {
    pub fn new() -> Self {
        let id = "TtT-1-0-0-TESTCLIENT".as_bytes().try_into().unwrap();

        let (sender, receiver) = mpsc::channel(100);
        let client = Client::new(id, receiver);

        let join_handle = tokio::spawn(async move {
            if let Err(e) = client.run().await {
                println!("Client error: {:?}", e);
            }
        });

        Self::setup_graceful_shutdown();

        Self {
            tx: sender,
            join_handle,
            id,
        }
    }

    pub async fn join(self) -> Result<()> {
        self.join_handle.await?;
        Ok(())
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

    pub async fn client_send_torrent_info(&mut self) -> Result<()> {
        let msg = ClientMessage::SendTorrentInfo;
        self.tx.send(msg).await.context("couldn't send a send torrent info message to the client")?;

        Ok(())
    }

    pub async fn client_terminal_closed(&mut self) -> Result<()> {
        let msg = ClientMessage::TerminalClientClosed;
        self.tx.send(msg).await.context("couldn't send a terminal client closed message to the client")?;

        Ok(())
    }
}
    
    
    