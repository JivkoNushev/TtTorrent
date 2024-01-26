use tokio::task::JoinHandle;
use tokio::sync::{mpsc, oneshot};
use anyhow::{Result, Context};

use crate::peer::peer_message::Handshake;
use crate::peer::{ConnectionType, PeerHandle, PeerMessage, PeerSession};
use crate::torrent::{Sha1Hash, TorrentContextState, TorrentHandle};
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

        for (_, val) in client_state.as_object().unwrap() { // client_state is always a valid json object
            let torrent_state = serde_json::from_value::<TorrentContextState>(val.clone())?;

            let torrent_handle = TorrentHandle::from_state(self.client_id, torrent_state, ConnectionType::Outgoing).await?;
            self.torrent_handles.push(torrent_handle);
        }

        Ok(())
    }


    #[tracing::instrument(
        name = "ClientHandler::run",
        skip(self),
        fields(
            client_id = String::from_utf8(self.client_id.to_vec()).unwrap_or_else(|_| "invalid client id".to_string())
        ),
    )]
    pub async fn run(mut self) -> Result<()> {
        tracing::event!(tracing::Level::INFO, "Client starting");

        let mut sending_interval = tokio::time::interval(std::time::Duration::from_secs(crate::INTERVAL_SECS));
        let mut sending_to_terminal_client = false;

        // load the client state
        self.load_state().await?;

        let seeding_socket = tokio::net::TcpListener::bind("0.0.0.0:".to_owned() + crate::SEEDING_PORT).await?;

        loop {
            tokio::select! {
                biased;
                Some(msg) = self.pipe.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            for torrent_handle in &mut self.torrent_handles {
                                let _ = torrent_handle.shutdown().await;
                            }
                            break;
                        },
                        ClientMessage::Download{src, dst} => {
                            let torrent_handle = TorrentHandle::new(self.client_id, &src, &dst).await?;
                            self.torrent_handles.push(torrent_handle);
                        },
                        ClientMessage::SendTorrentsInfo => {
                            sending_to_terminal_client = true;
                            println!("Sending torrents true");
                        },
                        ClientMessage::TerminalClientClosed => {
                            sending_to_terminal_client = false;
                        },
                        _ => {}
                    }
                }
                Ok((socket, _)) = seeding_socket.accept() => {
                    // TODO: use handshake::default
                    let mut peer_session = PeerSession::new(socket, ConnectionType::Incoming, Handshake::new(Sha1Hash([0; 20]), [0; 20])).await;

                    let handshake = peer_session.recv_handshake().await?;
                    let handshake = PeerMessage::as_handshake(&handshake)?;

                    peer_session.peer_handshake = handshake;

                    for torrent_handle in self.torrent_handles.iter_mut() {
                        if torrent_handle.torrent_info_hash == Sha1Hash::new(&peer_session.peer_handshake.info_hash) {
                            torrent_handle.add_peer_session(peer_session).await?;
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
                        torrent_handle.send_torrent_info(tx).await?;

                        torrent_rxs.push(rx);
                    }

                    for rx in torrent_rxs {
                        let torrent_state = rx.await?;
                        torrent_states.push(torrent_state);
                    }

                    if !torrent_states.is_empty() {
                        self.pipe.tx.send(ClientMessage::TorrentsInfo{torrents: torrent_states}).await?;
                    }
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