use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use anyhow::{Result, Context, Ok, anyhow};
use serde::{Serialize, Deserialize};

use std::sync::Arc;

use crate::messager::ClientMessage;
use crate::peer::{PeerHandle, PeerAddress, PeerTorrentContext};
use crate::tracker::Tracker;
use crate::peer::peer_message::ConnectionType;
use crate::disk_writer::{DiskWriterHandle, DiskWriterTorrentContext};
use crate::utils::CommunicationPipe;

pub mod torrent_file;
pub use torrent_file::{TorrentFile, Sha1Hash, BencodedValue};

pub mod torrent_parser;
pub use torrent_parser::TorrentParser;

#[derive(Debug, Serialize, Deserialize)]
pub struct TorrentState {
    src_path: String,
    dest_path: String,
    torrent_name: String,
    torrent_file: TorrentFile,
    info_hash: Sha1Hash,
    pieces: Vec<usize>,
    peers: Vec<PeerAddress>,
}

impl TorrentState {
    pub async fn new(torrent_context: TorrentContext) -> Self {
        Self {
            src_path: torrent_context.src_path,
            dest_path: torrent_context.dest_path,
            torrent_name: torrent_context.torrent_name,
            torrent_file: torrent_context.torrent_file,
            info_hash: torrent_context.info_hash,
            pieces: torrent_context.pieces.lock().await.clone(),
            peers: torrent_context.peers,
        }
    }
}

#[derive(Debug)]
pub struct TorrentContext {
    connection_type: ConnectionType,

    src_path: String,
    dest_path: String,
    torrent_name: String,
    torrent_file: TorrentFile,
    info_hash: Sha1Hash,
    pieces: Arc<Mutex<Vec<usize>>>,
    peers: Vec<PeerAddress>,
}

impl TorrentContext {
    pub fn from_state(torrent_state: TorrentState, connection_type: ConnectionType) -> Self {
        Self {
            connection_type,

            src_path: torrent_state.src_path,
            dest_path: torrent_state.dest_path,
            torrent_name: torrent_state.torrent_name,
            torrent_file: torrent_state.torrent_file,
            info_hash: torrent_state.info_hash,
            pieces: Arc::new(Mutex::new(torrent_state.pieces)),
            peers: torrent_state.peers,
        }
    }
}

struct Torrent {
    self_tx: mpsc::Sender<ClientMessage>,

    rx: mpsc::Receiver<ClientMessage>,
    peer_handles: Vec<PeerHandle>,
    disk_writer_handle: DiskWriterHandle,
    
    torrent_context: TorrentContext,
    client_id: [u8; 20],
}

// static functions for Torrent
impl Torrent {
    async fn save_state(torrent_context: TorrentContext) -> Result<()> {
        let connection_type = torrent_context.connection_type.clone();
        let torrent_context = TorrentState::new(torrent_context).await;
        
        let client_state = match tokio::fs::read_to_string(crate::STATE_FILE_PATH).await {
            std::result::Result::Ok(client_state) => client_state,
            Err(_) => String::new(),
        };

        let mut client_state: serde_json::Value = match client_state.len() {
            0 => serde_json::json!({}),
            _ => serde_json::from_str(&client_state).unwrap(),
        };

        let torrent_name = torrent_context.torrent_name.clone();

        if torrent_context.pieces.len() == 0 {
            let torrent_context = serde_json::to_value(torrent_context).unwrap();
            client_state["Downloaded"][torrent_name] = torrent_context;
        }
        else {
            let connection_type = match connection_type {
                ConnectionType::Incoming => "Seeding",
                ConnectionType::Outgoing => "Downloading",
            };

            let torrent_context = serde_json::to_value(torrent_context).unwrap();
            client_state[connection_type][torrent_name] = torrent_context;
        }

        let client_state = serde_json::to_string_pretty(&client_state).unwrap();

        tokio::fs::write(crate::STATE_FILE_PATH, client_state).await.context("couldn't write to client state file")?;

        Ok(())
    }
}

impl Torrent {
    pub async fn new(client_id: [u8; 20], self_pipe: CommunicationPipe, src: &str, dest: &str) -> Result<Self> {
        let torrent_file = TorrentFile::new(&src).await.context("couldn't create TorrentFile")?;

        let torrent_name = std::path::Path::new(src)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        let info_hash = TorrentFile::get_info_hash(torrent_file.get_bencoded_dict_ref())?;

        // not downloaded piece indexes
        let pieces_left = {
            let torrent_dict = torrent_file.get_bencoded_dict_ref().try_into_dict()?;
            let info_dict = match torrent_dict.get("info") {
                Some(info_dict) => info_dict,
                None => return Err(anyhow!("Could not get info dict from torrent file ref: {}", torrent_name))
            };
            let pieces = info_dict.get_from_dict("pieces")?;

            let pieces = match pieces {
                BencodedValue::ByteSha1Hashes(pieces) => pieces,
                _ => return Err(anyhow!("Could not get pieces from info dict ref in torrent file: {}", torrent_name))
            };

            let pieces_left = (0..pieces.len()).collect::<Vec<usize>>();

            pieces_left
        };

        let torrent_context = DiskWriterTorrentContext::new(
            dest.to_string(),
            torrent_name.to_string(),
            torrent_file.clone()
        );

        let disk_writer_handle = DiskWriterHandle::new(torrent_context, pieces_left.len());
        
        let torrent_context = TorrentContext {
            connection_type: ConnectionType::Outgoing, // TODO: make this configurable

            src_path: src.to_string(),
            dest_path: dest.to_string(),
            torrent_name: torrent_name.to_string(),
            torrent_file,
            info_hash,
            pieces: Arc::new(Mutex::new(pieces_left)),
            peers: Vec::new(),
        };

        Ok(Self {
            self_tx: self_pipe.tx,
            
            rx: self_pipe.rx,
            peer_handles: Vec::new(),
            disk_writer_handle,

            torrent_context,
            client_id,
        })
    }

    pub fn from_state(client_id: [u8; 20], self_pipe: CommunicationPipe, torrent_state: TorrentState, connection_type: ConnectionType) -> Result<Self> {
        let torrent_context = DiskWriterTorrentContext::new(
            torrent_state.dest_path.clone(),
            torrent_state.torrent_name.clone(),
            torrent_state.torrent_file.clone()
        );

        let torrent_pieces_count = torrent_state.pieces.len();
        
        let disk_writer_handle = DiskWriterHandle::new(torrent_context, torrent_pieces_count);
        
        let torrent_context = TorrentContext::from_state(torrent_state, connection_type);

        Ok(Self {
            self_tx: self_pipe.tx,

            rx: self_pipe.rx,
            peer_handles: Vec::new(),
            disk_writer_handle,

            torrent_context,
            client_id,
        })
    }   

    fn add_new_peers(&mut self, peer_addresses: Vec<PeerAddress>, connection_type: ConnectionType) -> Result<()>{
        let old_peer_addresses = self.peer_handles
        .iter()
        .map(|peer_handle| peer_handle.peer_address.clone())
        .collect::<Vec<PeerAddress>>();

        let peer_addresses = peer_addresses
        .iter()
        .filter(|peer_address| !old_peer_addresses.contains(peer_address))
        .cloned()
        .collect::<Vec<PeerAddress>>();

        let piece_length = self.torrent_context.torrent_file.get_piece_length()?;
        let torrent_length = self.torrent_context.torrent_file.get_torrent_length()?;

        for peer_address in peer_addresses {
            let peer_address_clone = peer_address.clone();
            if !self.torrent_context.peers.contains(&peer_address_clone) {
                self.torrent_context.peers.push(peer_address_clone);
            }

            let torrent_context = PeerTorrentContext::new(
                self.self_tx.clone(),
                piece_length,
                torrent_length,
                self.torrent_context.info_hash.clone(),
                Arc::clone(&self.torrent_context.pieces),
            );

            let peer_handle = PeerHandle::new(
                self.client_id,
                torrent_context,
                peer_address,
                connection_type.clone(),
            );

            self.peer_handles.push(peer_handle);
        }

        Ok(())
    }

    pub fn load_state(&mut self) -> Result<()> {
        self.add_new_peers(self.torrent_context.peers.clone(), self.torrent_context.connection_type.clone())
    }

    async fn get_tracker(&self) -> Result<Tracker> {
        let tracker_url = Tracker::tracker_url_get(self.torrent_context.torrent_file.get_bencoded_dict_ref())?;
        Ok(Tracker::new(&tracker_url).await)
    }

    async fn connect_to_peers(&mut self, tracker: &mut Tracker) -> Result<()> {
        // TODO: create a tracker request based on the missing pieces
        let tracker_response = tracker.default_params(&self.torrent_context.info_hash, self.client_id).await.context("couldn't get response with default parameters")?;
        let peer_addresses = match crate::DEBUG_MODE {
            true => vec![PeerAddress{address: "192.168.0.15".to_string(), port: "51413".to_string()}],
            false => PeerAddress::from_tracker_response(tracker_response).await
        };

        self.add_new_peers(peer_addresses, self.torrent_context.connection_type.clone())?;

        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
        self.load_state()?;

        // initialize tracker
        let mut tracker = self.get_tracker().await?;

        // if torrent needs more pieces
        if self.torrent_context.pieces.lock().await.len() > 0 {
            // connect to peers that may have the pieces
            self.connect_to_peers(&mut tracker).await?;
        }

        loop {
            tokio::select! {
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            // send stop message to all peers
                            for peer_handle in &mut self.peer_handles {
                                peer_handle.shutdown().await?;
                            }

                            // send stop message to disk writer
                            self.disk_writer_handle.shutdown().await?;

                            break;
                        }
                        ClientMessage::DownloadedPiece { piece_index, piece } => {
                            self.disk_writer_handle.downloaded_piece(piece_index, piece).await?;
                        },
                        _ => {}
                    }
                },
                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                    // connect to more peers with better tracker request
                    if self.torrent_context.pieces.lock().await.len() > 0 {
                        // connect to peers that may have the pieces
                        self.connect_to_peers(&mut tracker).await?;
                    }
                }
                // TODO: listen to an open socket for seeding
            }
        }

        for peer_handle in self.peer_handles {
            let _ = peer_handle.join().await;
        }

        self.disk_writer_handle.join().await?;

        println!("Torrent '{}' finished, now saving state", self.torrent_context.torrent_name);

        Torrent::save_state(self.torrent_context).await?;

        Ok(())
    }
    
}

pub struct TorrentHandle {
    tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<()>,

    torrent_name: String,
}

impl TorrentHandle {
    pub async fn new(client_id: [u8; 20], src: &str, dest: &str) -> Result<Self> {
        // copy torrent file to state folder
        let src_path = std::path::Path::new(src);
        let torrent_name = src_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        let dest_path = std::path::Path::new(crate::STATE_TORRENT_FILES_PATH).join(torrent_name);
        tokio::fs::copy(src_path, dest_path.clone()).await?;
        
        let src = dest_path.to_str().unwrap();
        let (sender, receiver) = mpsc::channel(100);

        let pipe = CommunicationPipe {
            tx: sender.clone(),
            rx: receiver,
        };
        let torrent = Torrent::new(client_id, pipe, src, dest).await?;

        let torrent_name = torrent.torrent_context.torrent_name.clone();

        let join_handle = tokio::spawn(async move {
            if let Err(e) = torrent.run().await {
                println!("Torrent error: {:?}", e);
            }
        });

        Ok(Self {
            tx: sender,
            join_handle,

            torrent_name,
        })
    }

    pub async fn from_state(client_id: [u8; 20], torrent_state: TorrentState, connection_type: ConnectionType) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(100);

        let pipe = CommunicationPipe {
            tx: sender.clone(),
            rx: receiver,
        };
        let torrent = Torrent::from_state(client_id, pipe, torrent_state, connection_type.clone())?;

        let torrent_name = torrent.torrent_context.torrent_name.clone();

        let join_handle = tokio::spawn(async move {
            if let Err(e) = torrent.run().await {
                println!("Torrent error: {:?}", e);
            }
        });

        Ok(Self {
            tx: sender,
            join_handle,

            torrent_name,
        })
    }

    pub async fn join(self) -> Result<()> {
        self.join_handle.await?;
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.tx.send(ClientMessage::Shutdown).await?;
        Ok(())
    }
}