use interprocess::local_socket::LocalSocketStream;
use tokio::task::JoinHandle;
use tokio::sync::mpsc;
use anyhow::{Result, Context};

use std::io::Write;

use crate::peer::ConnectionType;
use crate::torrent::{TorrentHandle, TorrentState};
use crate::messager::ClientMessage;

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

            let mut client_socket = LocalSocketStream::connect(crate::SOCKET_PATH).expect("Failed to connect to local socket");

            let serialized_data = serde_json::to_string(&ClientMessage::Shutdown).expect("Serialization failed");
            client_socket.write_all(serialized_data.as_bytes()).expect("Failed to write to socket");
        });
    }
}

impl ClientHandle {
    pub fn new() -> Self {
        // TODO: generate a random id
        let id = "TtT-1-0-0-TESTCLIENT".as_bytes().try_into().unwrap();

        let (sender, receiver) = mpsc::channel(crate::MAX_CHANNEL_SIZE);
        let client = Client::new(id, receiver);

        let join_handle = tokio::spawn(async move {
            if let Err(e) = client.run().await {
                eprintln!("Client error: {:?}", e);
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
        self.tx
        .send(ClientMessage::DownloadTorrent{src, dst})
        .await
        .context("couldn't send a download message to the client")?;

        Ok(())
    }

    pub async fn client_shutdown(&mut self) -> Result<()> {
        self.tx
        .send(ClientMessage::Shutdown)
        .await
        .context("couldn't send a shutdown message to the client")?;

        Ok(())
    }

    pub async fn client_send_torrent_info(&mut self) -> Result<()> {
        self.tx
        .send(ClientMessage::SendTorrentInfo)
        .await
        .context("couldn't send a send torrent info message to the client")?;

        Ok(())
    }

    pub async fn client_terminal_closed(&mut self) -> Result<()> {
        self.tx
        .send(ClientMessage::TerminalClientClosed)
        .await
        .context("couldn't send a terminal client closed message to the client")?;

        Ok(())
    }
}
    
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
                println!("No available client state");
                return Ok(());
            }
        };
        let client_state = serde_json::from_str::<serde_json::Value>(&client_state)?;
        println!("Loading client state");

        for (_, val) in client_state.as_object().unwrap() { // client_state is always a valid json object
            let torrent_state = serde_json::from_value::<TorrentState>(val.clone())?;

            let torrent_handle = TorrentHandle::from_state(self.client_id, torrent_state, ConnectionType::Outgoing).await?;
            self.torrent_handles.push(torrent_handle);
        }

        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
        let mut sending_to_terminal_client = false;

        // load the client state
        self.load_state().await?;

        loop {
            tokio::select! {
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::DownloadTorrent{src, dst} => {
                            let torrent_handle = TorrentHandle::new(self.client_id, &src, &dst).await?;
                            self.torrent_handles.push(torrent_handle);
                        },
                        ClientMessage::Shutdown => {
                            for torrent_handle in &mut self.torrent_handles {
                                let _ = torrent_handle.shutdown().await;
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

                    // TODO: save state to file
                    // self.save_state().await?;
                }
                else => {
                    break;
                }
            }
        }

        for torrent_handle in self.torrent_handles {
            if let Err(e) = torrent_handle.join().await {
                eprintln!("Failed to join torrent handle: {:?}", e);
            }
        }

        println!("Client stopping");

        Ok(())
    }
}