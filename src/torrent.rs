use tokio_stream::StreamExt;
use tokio::sync::{mpsc, oneshot, Mutex, };
use tokio::task::JoinHandle;
use anyhow::{anyhow, Result, Context};

use std::collections::HashMap;
use std::sync::Arc;

use crate::messager::ClientMessage;
use crate::peer::block_picker::Piece;
use crate::peer::{Block, BlockPicker, PeerAddress, PeerHandle, PeerSession, PeerTorrentContext};
use crate::tracker::{Tracker, TrackerEvent};
use crate::peer::peer_message::ConnectionType;
use crate::disk::{DiskHandle, DiskTorrentContext};
use crate::utils::CommunicationPipe;

pub mod torrent_file;
pub use torrent_file::{TorrentFile, Sha1Hash, BencodedValue};

pub mod torrent_parser;
pub use torrent_parser::TorrentParser;

pub mod torrent_info;
pub use torrent_info::TorrentInfo;

pub mod torrent_state;
pub use torrent_state::{TorrentContext, TorrentContextState};


pub struct TorrentHandle {
    tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<()>,

    pub _torrent_name: String,
    pub torrent_info_hash: Sha1Hash,
}

impl TorrentHandle {
    pub async fn new(client_id: [u8; 20], src: &str, dest: &str) -> Result<Self> {
        // ---------------------- copy torrent file to state folder for redundancy ----------------------
        let src_path = std::path::Path::new(src);
        let torrent_name = src_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        let torrent_file_dest_path = std::path::Path::new(crate::STATE_TORRENT_FILES_PATH).join(torrent_name);
        tokio::fs::copy(src_path, torrent_file_dest_path.clone()).await?;

        // --------------------------------- create torrent handle ---------------------------------
        let src = torrent_file_dest_path.to_str().unwrap(); // torrent_file_dest_path is always valid utf8
        
        let (sender, receiver) = mpsc::channel(crate::MAX_CHANNEL_SIZE);
        let pipe = CommunicationPipe {
            tx: sender.clone(),
            rx: receiver,
        };

        let torrent = match Torrent::new(client_id, pipe, src, dest).await {
            Ok(torrent) => torrent,
            Err(e) => {
                // remove torrent file from state folder
                tokio::fs::remove_file(src).await?;
                return Err(e);
            }
        };

        let _torrent_name = torrent.torrent_context.torrent_name.clone();
        let torrent_info_hash = torrent.torrent_context.info_hash.clone();

        let join_handle = tokio::spawn(async move {
            if let Err(e) = torrent.run().await {
                eprintln!("Torrent error: {:?}", e);
            }
        });

        Ok(Self {
            tx: sender,
            join_handle,

            _torrent_name,
            torrent_info_hash,
        })
    }

    pub async fn from_state(client_id: [u8; 20], torrent_state: TorrentContextState, info_hash: Sha1Hash, connection_type: ConnectionType) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(crate::MAX_CHANNEL_SIZE);
        let pipe = CommunicationPipe {
            tx: sender.clone(),
            rx: receiver,
        };

        let torrent = Torrent::from_state(client_id, pipe, torrent_state, info_hash, connection_type.clone()).await?;

        let _torrent_name = torrent.torrent_context.torrent_name.clone();
        let torrent_info_hash = torrent.torrent_context.info_hash.clone();

        let join_handle = tokio::spawn(async move {
            if let Err(e) = torrent.run().await {
                eprintln!("Torrent error: {:?}", e);
            }
        });

        Ok(Self {
            tx: sender,
            join_handle,

            _torrent_name,
            torrent_info_hash,
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

    pub async fn send_torrent_info(&mut self, tx: oneshot::Sender<TorrentContextState>) -> Result<()> {
        self.tx.send(ClientMessage::SendTorrentInfo{tx}).await?;
        Ok(())
    }

    pub async fn add_peer_session(&mut self, peer_session: PeerSession) -> Result<()> {
        self.tx.send(ClientMessage::AddPeerSession{peer_session}).await?;
        Ok(())
    }
}

struct Torrent {
    self_tx: mpsc::Sender<ClientMessage>,

    rx: mpsc::Receiver<ClientMessage>,
    peer_handles: Vec<PeerHandle>,
    disk_handle: DiskHandle,
    
    torrent_context: TorrentContext,
    client_id: [u8; 20],
}

// static functions for Torrent
impl Torrent {
    async fn save_state(torrent_context: TorrentContext) -> Result<()> {
        let state_file = std::path::Path::new(crate::STATE_FILE_PATH);
        let client_state = match tokio::fs::read_to_string(state_file).await {
            std::result::Result::Ok(client_state) => client_state,
            Err(_) => String::new(),
        };
        let mut client_state: serde_json::Value = match client_state.len() {
            0 => serde_json::json!({}),
            _ => serde_json::from_str(&client_state).unwrap(), // client state is always valid json
        };
        
        let torrent_info_hash = torrent_context.info_hash.clone();
        let torrent_context = TorrentContextState::new(torrent_context).await;
        
        let torrent_context = serde_json::to_value(torrent_context).unwrap(); // torrent context is always valid json
        
        client_state[torrent_info_hash.to_hex()] = torrent_context;

        let client_state = serde_json::to_string_pretty(&client_state).unwrap(); // client state is always valid json

        tokio::fs::write(state_file, client_state).await.context("couldn't write to client state file")?;

        Ok(())
    }
}


impl Torrent {
    pub async fn new(client_id: [u8; 20], self_pipe: CommunicationPipe, src: &str, dest: &str) -> Result<Self> {
        let torrent_fle_path = std::path::Path::new(src);

        let torrent_file = Arc::new(TorrentFile::new(torrent_fle_path).await.context("couldn't create TorrentFile")?);
        let torrent_info = Arc::new(TorrentInfo::new(&torrent_file)?);
        
        let torrent_name = std::path::Path::new(src)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap(); // src is always valid utf8

        let info_hash = TorrentFile::get_info_hash(torrent_file.get_bencoded_dict_ref())?;

        // not downloaded piece indexes
        let pieces_left = {
            let torrent_dict = torrent_file.get_bencoded_dict_ref().try_into_dict()?;
            let info_dict = match torrent_dict.get(&b"info".to_vec()) {
                Some(info_dict) => info_dict,
                None => return Err(anyhow!("Could not get info dict from torrent file ref: {}", torrent_name))
            };
            let pieces = info_dict.get_from_dict(b"pieces")?;

            let pieces = match pieces {
                BencodedValue::ByteSha1Hashes(pieces) => pieces,
                _ => return Err(anyhow!("Could not get pieces from info dict ref in torrent file: {}", torrent_name))
            };

            let pieces_left = (0u32..pieces.len() as u32)
                .map(|index| (index, Piece {
                    index,
                    block_count: torrent_info.get_specific_piece_block_count(index),
                })).collect::<HashMap<u32, Piece>>().into_iter()
                .map(|(_, piece)| piece).collect::<Vec<Piece>>();

            pieces_left
        };

        let pieces_count = pieces_left.len();
        
        let torrent_context = DiskTorrentContext::new(
            self_pipe.tx.clone(),
            dest.to_string(),
            torrent_name.to_string(),
            Arc::clone(&torrent_file),
            Arc::clone(&torrent_info),
        );

        let disk_handle = DiskHandle::new(torrent_context);

        let needed = BlockPicker::new(pieces_left, Arc::clone(&torrent_info));

        let torrent_context = TorrentContext {
            connection_type: ConnectionType::Outgoing,

            src_path: src.to_string(),
            dest_path: dest.to_string(),
            torrent_name: torrent_name.to_string(),
            torrent_file: torrent_file,
            info_hash,
            needed: Arc::new(Mutex::new(needed)),
            bitfield: Arc::new(Mutex::new(vec![0; pieces_count.div_ceil(8)])),
            peers: Vec::new(),

            torrent_info,

            downloaded: Arc::new(Mutex::new(0)),
            uploaded: Arc::new(Mutex::new(0)),
        };

        Ok(Self {
            self_tx: self_pipe.tx,
            
            rx: self_pipe.rx,
            peer_handles: Vec::new(),
            disk_handle,

            torrent_context,
            client_id,
        })
    }


    pub async fn from_state(client_id: [u8; 20], self_pipe: CommunicationPipe, torrent_state: TorrentContextState, info_hash: Sha1Hash, connection_type: ConnectionType) -> Result<Self> {
        let torrent_file_path = format!("{}/{}.torrent", crate::STATE_TORRENT_FILES_PATH, torrent_state.torrent_name);
        let path = std::path::Path::new(&torrent_file_path);

        let torrent_file = TorrentFile::new(path).await.context("couldn't create TorrentFile")?;
        
        let disk_torrent_context = DiskTorrentContext::new(
            self_pipe.tx.clone(),
            torrent_state.dest_path.clone(),
            torrent_state.torrent_name.clone(),
            Arc::new(torrent_file),
            Arc::new(torrent_state.torrent_info.clone()),
        );
        
        let disk_handle = DiskHandle::new(disk_torrent_context);
        
        let torrent_context = TorrentContext::from_state(torrent_state, info_hash, connection_type).await?;
        Ok(Self {
            self_tx: self_pipe.tx,

            rx: self_pipe.rx,
            peer_handles: Vec::new(),
            disk_handle,

            torrent_context,
            client_id,
        })
    }   

    async fn add_new_peers(&mut self, peer_addresses: Vec<PeerAddress>, connection_type: ConnectionType) -> Result<()> {
        let old_peer_addresses = self.peer_handles
            .iter()
            .map(|peer_handle| peer_handle.peer_address.clone())
            .collect::<Vec<PeerAddress>>();

        let peer_addresses = peer_addresses
            .iter()
            .filter(|peer_address| !old_peer_addresses.contains(peer_address))
            .cloned()
            .collect::<Vec<PeerAddress>>();

        let mut peer_addresses_iter = tokio_stream::iter(peer_addresses);
        while let Some(peer_address) = peer_addresses_iter.next().await {
            if !self.torrent_context.peers.contains(&peer_address) {
                self.torrent_context.peers.push(peer_address.clone());
            }

            let torrent_context = PeerTorrentContext::new(
                self.self_tx.clone(),
                Arc::clone(&self.torrent_context.torrent_info),
                self.torrent_context.info_hash.clone(),
                Arc::clone(&self.torrent_context.needed),
                Arc::clone(&self.torrent_context.bitfield),
                Arc::clone(&self.torrent_context.uploaded),
                Arc::clone(&self.torrent_context.downloaded),
            );

            let peer_handle = PeerHandle::new(
                self.client_id,
                torrent_context,
                peer_address,
                connection_type.clone(),
                self.disk_handle.tx.clone(),
            ).await?;

            self.peer_handles.push(peer_handle);
        }

        Ok(())
    }

    pub fn load_state(&mut self) {

        // I could try to connect to the peers from the previous session but I don't think it's worth it
        // because the tracker will return me the best peers to connect to
        // self.add_new_peers(self.torrent_context.peers.clone(), self.torrent_context.connection_type.clone()).await
        
        // removing the previous peers who participated
        self.torrent_context.peers.clear();
    }

    async fn connect_to_peers(&mut self, tracker: &mut Tracker) -> Result<()> {
        let peer_addresses = match crate::DEBUG_MODE {
            true => {
                // vec![PeerAddress{address: "192.168.0.24".to_string(), port: "6969".to_string()}]
                // vec![PeerAddress{address: "127.0.0.1".to_string(), port: "51413".to_string()}, PeerAddress{address: "192.168.0.24".to_string(), port: "51413".to_string()}]
                vec![PeerAddress{address: "192.168.0.24".to_string(), port: "6969".to_string()}, PeerAddress{address: "127.0.0.1".to_string(), port: "51413".to_string()}, PeerAddress{address: "192.168.0.24".to_string(), port: "51413".to_string()}]
            },
            false => {
                let tracker_response = match self.torrent_context.needed.lock().await.pieces.len() == self.torrent_context.torrent_info.pieces_count {
                    true => tracker.response(self.client_id.clone(), &self.torrent_context, TrackerEvent::Started).await?,
                    false => tracker.response(self.client_id.clone(), &self.torrent_context, TrackerEvent::None).await?,
                };
                PeerAddress::from_tracker_response(tracker_response).await?
            }
        };

        let peer_addresses = peer_addresses.into_iter().rev().take(10).collect();

        self.add_new_peers(peer_addresses, self.torrent_context.connection_type.clone()).await?;

        Ok(())
    }

    pub async fn tracker_stopped(&mut self, tracker: &mut Tracker) -> Result<()> {
        if !crate::DEBUG_MODE {
            tracker.response(self.client_id.clone(), &self.torrent_context, TrackerEvent::Stopped).await.context("couldn't get tracker response")?;
        }

        Ok(())
    }

    async fn tracker_completed(&mut self, tracker: &mut Tracker) -> Result<()> {
        if !crate::DEBUG_MODE {
            tracker.response(self.client_id.clone(), &self.torrent_context, TrackerEvent::Completed).await.context("couldn't get tracker response")?;
        }

        Ok(())
    }   

    #[tracing::instrument(
        name = "Torrent::run",
        skip(self),
        fields(
            torrent_name = self.torrent_context.torrent_name.as_str(),
            info_hash = self.torrent_context.info_hash.to_hex().as_str(),
            peers = self.torrent_context.peers.len(),
        )
    )]
    pub async fn run(mut self) -> Result<()> {
        self.load_state();

        // ------------------------------ create tracker --------------------------------
        let mut tracker = match Tracker::from_torrent_file(&self.torrent_context.torrent_file) {
            Ok(tracker) => Some(tracker),
            Err(e) => {
                tracing::error!("Failed to create tracker: {}", e);
                None
            }
        };

        // ------------------------------ connect to peers --------------------------------
        if let Some(ref mut tracker) = tracker {
            // connect to peers from tracker response
            if let Err(e) = self.connect_to_peers(tracker).await {
                tracing::error!("Failed to connect to peers: {}", e);
            }
        }
        
        // ------------------------------ main loop --------------------------------
        let mut end_game_blocks: Vec<Block> = Vec::new();
        loop {
            tokio::select! {
                biased;

                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            tracing::info!("Shutting down torrent '{}'", self.torrent_context.torrent_name);
                            for peer_handle in &mut self.peer_handles {
                                if let Err(e) = peer_handle.shutdown().await {
                                    tracing::warn!("Failed to send shutdown message to peer {}: {}", peer_handle.peer_address, e);
                                }
                            }

                            if let Err(e) = self.disk_handle.shutdown().await {
                                tracing::warn!("Failed to send shutdown message to disk handle: {}", e);
                            }

                            if let Some(ref mut tracker) = tracker {
                                // TODO: is this to be called before the handles are joined?
                                if let Err(e) = self.tracker_stopped(tracker).await {
                                    tracing::warn!("Failed to send stopped message to tracker: {}", e);
                                }
                            }
                            break;
                        },
                        ClientMessage::Have { piece } => {
                            tracing::debug!("Have piece: {}", piece);                     
                            self.torrent_context.bitfield.lock().await[piece as usize / 8] |= 1 << (7 - piece % 8);  

                            // TODO: This breaks the program
                            // for peer_handle in &mut self.peer_handles {
                            //     let _ = peer_handle.have(piece).await;
                            // }   
                        },
                        ClientMessage::Cancel { block } => {
                            if !end_game_blocks.iter().any(|b| b.index == block.index && b.begin == block.begin && b.length == block.length){
                                
                                let block_copy = Block {
                                    index: block.index,
                                    begin: block.begin,
                                    length: block.length,

                                    number: 0, // doesn't matter
                                    data: None, // None is simpler to copy
                                };

                                {
                                    // removing piece from block picker when all blocks are downloaded
                                    let end_game_current_piece_blocks_count = end_game_blocks.iter().filter(|b| b.index == block.index).count() + 1;
                                    
                                    let mut needed_guard = self.torrent_context.needed.lock().await;
                                    let position = match needed_guard.pieces.iter().position(|p| p.index == block.index) {
                                        Some(position) => position,
                                        None => {
                                            tracing::error!("Block picker already removed end game piece {}.", block.index);
                                            continue;
                                        }
                                    };

                                    if end_game_current_piece_blocks_count == needed_guard.pieces[position].block_count {
                                        needed_guard.pieces.remove(position);
                                    }   
                                }

                                *self.torrent_context.downloaded.lock().await += block.length as u64;
                                self.disk_handle.write_block(block).await?;

                                // for peer_handle in &mut self.peer_handles {
                                //     if let Err(e) = peer_handle.cancel(block_copy.clone()).await {
                                //         tracing::warn!("Failed to send cancel message to peer {}: {}", peer_handle.peer_address, e);
                                //     }
                                // }
                                end_game_blocks.push(block_copy);
                            }
                        },
                        ClientMessage::Request { block, tx } => {
                            if let Err(e) = self.disk_handle.read_block(block, tx).await.context("sending to disk handle") {
                                tracing::error!("Failed to send block request to disk handle: {}", e);
                            }
                        },
                        ClientMessage::FinishedDownloading => {
                            if let Err(e) = Torrent::save_state(self.torrent_context.clone()).await.context("saving torrent state") {
                                tracing::error!("Failed to save torrent state for torrent {}: {}", self.torrent_context.torrent_name, e);
                            }
                            tracing::info!("Finished downloading torrent '{}'", self.torrent_context.torrent_name);

                            if let Some(ref mut tracker) = tracker {
                                if let Err(e) = self.tracker_completed(tracker).await {
                                    tracing::error!("Failed to send completed message to tracker: {}", e);
                                }
                            } 
                        },
                        ClientMessage::SendTorrentInfo { tx } => {
                            let torrent_state = TorrentContextState::new(self.torrent_context.clone()).await;
                            if let Err(e) = tx.send(torrent_state) {
                                tracing::error!("Failed to send torrent context to client: {:?}", e);
                            }
                        },
                        ClientMessage::PeerDisconnected { peer_address } => {
                            self.torrent_context.peers.retain(|peer| peer != &peer_address);

                            let handle_index = self.peer_handles.iter().position(|peer_handle| peer_handle.peer_address == peer_address);
                            if let Some(handle_index) = handle_index {
                                if let Err(e) = self.peer_handles.remove(handle_index).join().await {
                                    tracing::error!("Failed to join peer handle: {}", e);
                                }
                            }
                        },
                        ClientMessage::AddPeerSession { peer_session } => {
                            let peer_address = match peer_session.stream.peer_addr() {
                                Ok(peer_address) => peer_address,
                                Err(e) => {
                                    tracing::error!("Failed to get peer address: {}", e);
                                    continue;
                                }
                            };
                            let peer_address = PeerAddress {
                                address: peer_address.ip().to_string(),
                                port: peer_address.port().to_string(),
                            };
                            
                            if self.torrent_context.peers.contains(&peer_address) {
                                continue;
                            }

                            let torrent_context = PeerTorrentContext::new(
                                self.self_tx.clone(),
                                Arc::clone(&self.torrent_context.torrent_info),
                                self.torrent_context.info_hash.clone(),
                                Arc::clone(&self.torrent_context.needed),
                                Arc::clone(&self.torrent_context.bitfield),
                                Arc::clone(&self.torrent_context.uploaded),
                                Arc::clone(&self.torrent_context.downloaded)
                            );

                            let peer_handle = match PeerHandle::from_session(
                                self.client_id,
                                torrent_context,
                                peer_session,
                                self.disk_handle.tx.clone(),
                            ).await {
                                Ok(peer_handle) => peer_handle,
                                Err(e) => {
                                    tracing::error!("Failed to create peer handle: {}", e);
                                    continue;
                                }
                            };

                            self.peer_handles.push(peer_handle);
                        }
                        _ => {}
                    }
                },
                _ = tokio::time::sleep(std::time::Duration::from_secs(120)) => {
                    // connect to more peers with better tracker request
                    if !self.torrent_context.needed.lock().await.is_empty() {
                        if let Some(ref mut tracker) = tracker {
                            // connect to peers from tracker response
                            if let Err(e) = self.connect_to_peers(tracker).await {
                                tracing::error!("Failed to connect to peers: {}", e);
                            }
                        }
                    }
                }
            }
        }

        for peer_handle in self.peer_handles {
            let peer_addr = peer_handle.peer_address.clone();
            if let Err(err) = peer_handle.join().await {
                if let Some(err) = err.downcast_ref::<tokio::io::Error>() {
                    match err.kind() {
                        tokio::io::ErrorKind::UnexpectedEof => {
                            self.torrent_context.peers.retain(|peer| peer != &peer_addr);
                            tracing::debug!("Peer '{}' UnexpectedEof", peer_addr);
                        }
                        tokio::io::ErrorKind::ConnectionReset => {
                            self.torrent_context.peers.retain(|peer| peer != &peer_addr);
                            tracing::debug!("Peer '{}' ConnectionReset", peer_addr);
                        }
                        tokio::io::ErrorKind::ConnectionAborted => {
                            self.torrent_context.peers.retain(|peer| peer != &peer_addr);
                            tracing::debug!("Peer '{}' ConnectionAborted", peer_addr);
                        }
                        _ => {
                            tracing::warn!("Peer '{}' error: {}", peer_addr, err);
                        }
                    }
                }
            }
        }

        self.disk_handle.join().await?;
        Torrent::save_state(self.torrent_context).await?;

        Ok(())
    }
    
}