use tokio::task::JoinHandle;
use tokio::sync::{mpsc, oneshot};
use anyhow::{Result, Context};

use crate::peer::peer_message::Handshake;
use crate::peer::{ConnectionType, PeerMessage, PeerSession};
use crate::torrent::{Sha1Hash, TorrentContextState, TorrentHandle};
use crate::messager::ClientMessage;
use crate::utils::{CommunicationPipe, UrlEncodable};

pub struct ClientHandle {
    tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<()>,

    _id: [u8; 20],
}

impl ClientHandle {
    pub fn new(tx: mpsc::Sender<ClientMessage>) -> Self {
        // let id = "TtT-1-0-0-TESTCLIENT".as_bytes().try_into().unwrap();
        let id = crate::utils::generate_random_client_id();

        let (sender, receiver) = mpsc::channel(crate::MAX_CHANNEL_SIZE);
        
        let pipe = CommunicationPipe{tx, rx: receiver};
        let client = Client::new(id, pipe);

        let join_handle = tokio::spawn(async move {
            if let Err(e) = client.run().await {
                tracing::error!("Client::run failed: {:?}", e);
            }
        });

        Self {
            tx: sender,
            join_handle,
            _id: id,
        }
    }

    pub async fn join(self) -> Result<()> {
        self.join_handle.await?;
        Ok(())
    }

    pub async fn client_add_torrent(&mut self, src: String, dst: String) -> Result<()> {
        self.tx
            .send(ClientMessage::AddTorrent{src, dst})
            .await
            .context("couldn't send an add message to the client")?;

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
        let path = std::path::Path::new(crate::STATE_FILE_PATH);

        let client_state = match tokio::fs::read_to_string(path).await {
            Ok(state) => state,
            Err(_) => {
               // println!("No available client state");
                return Ok(());
            }
        };
        let client_state = serde_json::from_str::<serde_json::Value>(&client_state)?;
       // println!("Loading client state");

        for (key, val) in client_state.as_object().unwrap() { // client_state is always a valid json object
            let torrent_state = serde_json::from_value::<TorrentContextState>(val.clone())?;

            let info_hash = match Sha1Hash::from_hex(key) {
                Ok(info_hash) => info_hash,
                Err(e) => {
                    tracing::error!("Failed to convert info hash from hex: {:?}", e);
                    continue;
                }
            };

            let torrent_handle = TorrentHandle::from_state(self.client_id, torrent_state, info_hash, ConnectionType::Outgoing).await?;
            self.torrent_handles.push(torrent_handle);
        }

        Ok(())
    }


    #[tracing::instrument(
        name = "ClientHandler::run",
        skip(self),
        fields(
            client_id = self.client_id.as_url_encoded()
        ),
    )]
    pub async fn run(mut self) -> Result<()> {
        tracing::event!(tracing::Level::INFO, "Client starting");

        // load the client state
        self.load_state().await?;
        
        let mut sending_interval = tokio::time::interval(std::time::Duration::from_secs(crate::INTERVAL_SECS));
        let mut sending_to_terminal_client = false;

        let seeding_socket = tokio::net::TcpListener::bind("0.0.0.0:".to_owned() + &crate::SEEDING_PORT.to_string()).await?;
        
        loop {
            tokio::select! {
                biased;

                Some(msg) = self.pipe.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            for torrent_handle in &mut self.torrent_handles {
                                if let Err(e) = torrent_handle.shutdown().await {
                                    tracing::error!("Failed to shutdown torrent handle: {:?}", e);
                                }
                            }
                            break;
                        },
                        ClientMessage::AddTorrent{src, dst} => {
                            let mut torrent_handle = match TorrentHandle::new(self.client_id, &src, &dst).await {
                                Ok(handle) => handle,
                                Err(e) => {
                                    tracing::error!("Failed to create torrent handle: {:?}", e);
                                    continue;
                                }
                            };

                            if self.torrent_handles.iter().any(|handle| handle.torrent_info_hash == torrent_handle.torrent_info_hash) {
                                torrent_handle.shutdown().await?;
                                torrent_handle.join().await?;
                                continue;
                            }
                            else {
                                self.torrent_handles.push(torrent_handle);
                            }
                        },
                        ClientMessage::SendTorrentsInfo => {
                            sending_to_terminal_client = true;
                            tracing::debug!("Sending torrents info to terminal clients");
                        },
                        ClientMessage::TerminalClientClosed => {
                            sending_to_terminal_client = false;
                            tracing::debug!("Stopped sending torrents info to terminal clients");
                        },
                        _ => {
                            tracing::warn!("Received unimportant message in client: {:?}", msg);
                        },
                    }
                }
                Ok((socket, _)) = seeding_socket.accept() => {
                    let mut peer_session = PeerSession::new(socket, ConnectionType::Incoming, Handshake::default()).await;

                    peer_session.peer_handshake = match peer_session.recv_handshake().await {
                        Ok(handshake) => {
                            match PeerMessage::as_handshake(&handshake) {
                                Ok(handshake) => handshake,
                                Err(e) => {
                                    tracing::error!("Failed to convert peer message to handshake: {:?}", e);
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to receive handshake from peer incoming connection: {:?}", e);
                            continue;
                        }
                    };

                    for torrent_handle in self.torrent_handles.iter_mut() {
                        if torrent_handle.torrent_info_hash.as_bytes() == &peer_session.peer_handshake.info_hash {
                            if let Err(e) = torrent_handle.add_peer_session(peer_session).await {
                                tracing::error!("Failed to add peer session to torrent handle: {:?}", e);
                            }
                            break;
                        }
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
                        if let Err(e) = torrent_handle.send_torrent_info(tx).await {
                            tracing::error!("Failed to send torrent info to torrent handle: {:?}", e);
                            continue;
                        }

                        torrent_rxs.push(rx);
                    }

                    for rx in torrent_rxs {
                        let torrent_state = match rx.await {
                            Ok(state) => state,
                            Err(e) => {
                                tracing::error!("Failed to receive torrent state from torrent handle: {:?}", e);
                                continue;
                            }
                        };

                        torrent_states.push(torrent_state);
                    }

                    if !torrent_states.is_empty() {
                        if let Err(e) = self.pipe.tx.send(ClientMessage::TorrentsInfo{torrents: torrent_states}).await {
                            tracing::error!("Failed to send torrents info to terminal client: {:?}", e);
                        }
                    }
                }
            }
        }

        for torrent_handle in self.torrent_handles {
            if let Err(e) = torrent_handle.join().await {
                tracing::error!("Failed to join torrent handle: {:?}", e);
            }
        }

        tracing::event!(tracing::Level::INFO, "Client gracefull shutdown");

        Ok(())
    }
}