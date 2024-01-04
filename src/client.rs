use tokio::task::JoinHandle;
use tokio::sync::{mpsc, oneshot};
use anyhow::{Result, Context};
use tokio::time::interval;

use crate::peer::ConnectionType;
use crate::torrent::{TorrentHandle, TorrentState};
use crate::messager::ClientMessage;
use crate::utils::CommunicationPipe;

pub struct ClientHandle {
    tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<()>,

    id: [u8; 20],
}

impl ClientHandle {
    pub fn new(tx: mpsc::Sender<ClientMessage>) -> Self {
        // TODO: generate a random id
        let id = "TtT-1-0-0-TESTCLIENT".as_bytes().try_into().unwrap();

        let (sender, receiver) = mpsc::channel(crate::MAX_CHANNEL_SIZE);
        
        let pipe = CommunicationPipe{tx, rx: receiver};
        let client = Client::new(id, pipe);

        let join_handle = tokio::spawn(async move {
            if let Err(e) = client.run().await {
                eprintln!("Client error: {:?}", e);
            }
        });

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
        .send(ClientMessage::Download{src, dst})
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

    pub async fn client_list_torrents(&mut self) -> Result<()> {
        self.tx
        .send(ClientMessage::SendTorrentsInfo)
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
    pipe: CommunicationPipe,
    torrent_handles: Vec<TorrentHandle>,

    client_id: [u8; 20],
}

impl Client {
    pub fn new(client_id: [u8; 20], pipe: CommunicationPipe) -> Self {
        Self {
            pipe,
            torrent_handles: Vec::new(),

            client_id,
        }
    }

    async fn load_state(&mut self) -> Result<()> {
        let client_state = match tokio::fs::read_to_string(crate::STATE_FILE_PATH).await {
            Ok(state) => state,
            Err(_) => {
               // println!("No available client state");
                return Ok(());
            }
        };
        let client_state = serde_json::from_str::<serde_json::Value>(&client_state)?;
       // println!("Loading client state");

        for (_, val) in client_state.as_object().unwrap() { // client_state is always a valid json object
            let torrent_state = serde_json::from_value::<TorrentState>(val.clone())?;

            let torrent_handle = TorrentHandle::from_state(self.client_id, torrent_state, ConnectionType::Outgoing).await?;
            self.torrent_handles.push(torrent_handle);
        }

        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
        let mut sending_interval = tokio::time::interval(std::time::Duration::from_secs(crate::INTERVAL_SECS));
        let mut sending_to_terminal_client = false;

        // load the client state
        self.load_state().await?;


        loop {
            tokio::select! {
                Some(msg) = self.pipe.rx.recv() => {
                    match msg {
                        ClientMessage::Download{src, dst} => {
                            let torrent_handle = TorrentHandle::new(self.client_id, &src, &dst).await?;
                            self.torrent_handles.push(torrent_handle);
                        },
                        ClientMessage::Shutdown => {
                            for torrent_handle in &mut self.torrent_handles {
                                let _ = torrent_handle.shutdown().await;
                            }
                            break;
                        },
                        ClientMessage::SendTorrentsInfo => {
                            sending_to_terminal_client = true;
                        },
                        ClientMessage::TerminalClientClosed => {
                            sending_to_terminal_client = false;
                        },
                        _ => {}
                    }
                }
                _ = sending_interval.tick() => {
                    if !sending_to_terminal_client {
                        continue;
                    }

                    let mut torrent_states = Vec::new();
                    let mut torrent_rxs = Vec::new();

                    for torrent_handle in &mut self.torrent_handles {
                        let (tx, rx) = oneshot::channel();
                        torrent_handle.send_torrent_info(tx).await?;

                        torrent_rxs.push(rx);
                    }

                    for rx in torrent_rxs {
                        let torrent_state = rx.await?;
                        torrent_states.push(torrent_state);
                    }

                    self.pipe.tx.send(ClientMessage::TorrentsInfo{torrents: torrent_states}).await?;
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

       // println!("Client stopping");

        Ok(())
    }
}