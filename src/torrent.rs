use futures::StreamExt;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use anyhow::{anyhow, Result, Context};

use std::sync::Arc;

use crate::messager::ClientMessage;
use crate::peer::{PeerAddress, PeerHandle, PeerTorrentContext, BlockPicker};
use crate::tracker::Tracker;
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

    torrent_name: String,
}

impl TorrentHandle {
    pub async fn new(client_id: [u8; 20], src: &str, dest: &str) -> Result<Self> {
        // copy torrent file to state folder for redundancy
        let src_path = std::path::Path::new(src);
        let torrent_name = src_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        let torrent_file_dest_path = std::path::Path::new(crate::STATE_TORRENT_FILES_PATH).join(torrent_name);
        tokio::fs::copy(src_path, torrent_file_dest_path.clone()).await?;

        // create torrent handle
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
        let torrent_name = torrent.torrent_context.torrent_name.clone();

        let join_handle = tokio::spawn(async move {
            if let Err(e) = torrent.run().await {
                eprintln!("Torrent error: {:?}", e);
            }
        });

        Ok(Self {
            tx: sender,
            join_handle,

            torrent_name,
        })
    }

    pub async fn from_state(client_id: [u8; 20], torrent_state: TorrentContextState, connection_type: ConnectionType) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(crate::MAX_CHANNEL_SIZE);

        let pipe = CommunicationPipe {
            tx: sender.clone(),
            rx: receiver,
        };
        let torrent = Torrent::from_state(client_id, pipe, torrent_state, connection_type.clone())?;

        let torrent_name = torrent.torrent_context.torrent_name.clone();

        let join_handle = tokio::spawn(async move {
            if let Err(e) = torrent.run().await {
                eprintln!("Torrent error: {:?}", e);
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

    pub async fn send_torrent_info(&mut self, tx: oneshot::Sender<TorrentContextState>) -> Result<()> {
        self.tx.send(ClientMessage::SendTorrentInfo{tx}).await?;
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
        let torrent_context = TorrentContextState::new(torrent_context).await;

        let path = std::path::Path::new(crate::STATE_FILE_PATH);
        let client_state = match tokio::fs::read_to_string(path).await {
            std::result::Result::Ok(client_state) => client_state,
            Err(_) => String::new(),
        };

        let mut client_state: serde_json::Value = match client_state.len() {
            0 => serde_json::json!({}),
            _ => serde_json::from_str(&client_state).unwrap(), // client state is always valid json
        };

        let torrent_info_hash = torrent_context.info_hash.clone();
        let torrent_context = serde_json::to_value(torrent_context).unwrap(); // torrent context is always valid json
        client_state[torrent_info_hash.to_hex()] = torrent_context;

        let client_state = serde_json::to_string_pretty(&client_state).unwrap(); // client state is always valid json

        tokio::fs::write(path, client_state).await.context("couldn't write to client state file")?;

        Ok(())
    }
}


impl Torrent {
    pub async fn new(client_id: [u8; 20], self_pipe: CommunicationPipe, src: &str, dest: &str) -> Result<Self> {
        let torrent_file = Arc::new(TorrentFile::new(&src).await.context("couldn't create TorrentFile")?);
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

        let pieces_count = pieces_left.len();

        let blocks_left = {
            let mut blocks_left = Vec::new();
            let blocks_in_piece = torrent_file.get_piece_size(0)?.div_ceil(crate::BLOCK_SIZE);

            for piece in pieces_left.iter() {
                let piece_size = torrent_file.get_piece_size(*piece)?;
                let blocks_count = piece_size.div_ceil(crate::BLOCK_SIZE);

                for block in 0..blocks_count {
                    let block_index = *piece * blocks_in_piece + block;
                    blocks_left.push(block_index);
                }
            }

            blocks_left
        };

        let torrent_context = DiskTorrentContext::new(
            self_pipe.tx.clone(),
            dest.to_string(),
            torrent_name.to_string(),
            Arc::clone(&torrent_file),
            Arc::clone(&torrent_info),
        );

        let disk_handle = DiskHandle::new(torrent_context, blocks_left.len());
        

        let needed = BlockPicker::new(blocks_left, pieces_left, Arc::clone(&torrent_info));

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

            downloaded: 0,
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


    pub fn from_state(client_id: [u8; 20], self_pipe: CommunicationPipe, torrent_state: TorrentContextState, connection_type: ConnectionType) -> Result<Self> {
        let disk_torrent_context = DiskTorrentContext::new(
            self_pipe.tx.clone(),
            torrent_state.dest_path.clone(),
            torrent_state.torrent_name.clone(),
            Arc::new(torrent_state.torrent_file.clone()),
            Arc::new(torrent_state.torrent_info.clone()),
        );
        let torrent_blocks_count = torrent_state.needed.needed_blocks.len();
        
        let disk_handle = DiskHandle::new(disk_torrent_context, torrent_blocks_count);
        
        let torrent_context = TorrentContext::from_state(torrent_state, connection_type);
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
            );

            let peer_handle = PeerHandle::new(
                self.client_id,
                torrent_context,
                peer_address,
                connection_type.clone(),
            ).await?;

            self.peer_handles.push(peer_handle);
        }

        Ok(())
    }

    pub async fn load_state(&mut self) -> Result<()> {
        self.add_new_peers(self.torrent_context.peers.clone(), self.torrent_context.connection_type.clone()).await
    }

    async fn connect_to_peers(&mut self, tracker: &mut Tracker) -> Result<()> {
        let tracker_response = tracker.regular_response(self.client_id.clone(), &self.torrent_context).await?;

        let peer_addresses = match crate::DEBUG_MODE {
            true => vec![PeerAddress{address: "127.0.0.1".to_string(), port: "51413".to_string()}, PeerAddress{address: "192.168.0.24".to_string(), port: "51413".to_string()}],
            false => PeerAddress::from_tracker_response(tracker_response).await?
        };

        // println!("peer addresses: {:?}", peer_addresses);
        let peer_addresses = peer_addresses.into_iter().rev().take(10).collect();
        self.add_new_peers(peer_addresses, self.torrent_context.connection_type.clone()).await?;

        Ok(())
    }

    pub async fn tracker_stopped(&mut self, tracker: &mut Tracker) -> Result<()> {
        let _ = tracker.stopped_response(self.client_id.clone(), &self.torrent_context).await.context("couldn't get tracker response")?;

        Ok(())
    }

    async fn tracker_completed(&mut self, tracker: &mut Tracker) -> Result<()> {
        let _ = tracker.completed_response(self.client_id.clone(), &self.torrent_context).await.context("couldn't get tracker response")?;

        Ok(())
    }   

    pub async fn run(mut self) -> Result<()> {
        self.load_state().await?;

        // initialize tracker
        let mut tracker = match Tracker::from_torrent_file(&self.torrent_context.torrent_file) {
            Ok(tracker) => Some(tracker),
            Err(e) => {
                eprintln!("Failed to create tracker: {}", e);
                None
            }
        };

        if let Some(ref mut tracker) = tracker {
            // connect to peers from tracker response
            if let Err(e) = self.connect_to_peers(tracker).await {
                eprintln!("Failed to connect to peers: {}", e);
            }
        }

        // let mut last_blocks = Vec::new();

        loop {
            tokio::select! {
                biased;
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            // send stop message to all peers
                            for peer_handle in &mut self.peer_handles {
                                let _ = peer_handle.shutdown().await;
                            }
                            // send stop message to disk writer
                            let _ = self.disk_handle.shutdown().await;

                            if let Some(ref mut tracker) = tracker {
                                // TODO: is this to be called before the handles are joined?
                                if let Err(e) = self.tracker_stopped(tracker).await {
                                    eprintln!("Failed to send stopped message to tracker: {}", e);
                                }
                            }
                            break;
                        },
                        ClientMessage::DownloadedBlock { piece_block } => {
                            // let mut l = self.torrent_context.needed_blocks.lock().await;
                            // if l.contains(&block.block_index) {
                            //     l.retain(|index| index != &block.block_index);
                            //     for peer_handle in &mut self.peer_handles {
                            //         let _ = peer_handle.cancel_block(block.block_index as u32, block.offset as u32, block.size as u32).await;
                            //     }
                            //     last_blocks.push(block.block_index);
                            // }
                            // else if last_blocks.contains(&block.block_index) {
                            //     continue;
                            // }
                            self.torrent_context.downloaded += piece_block.size as u64;
                            self.disk_handle.write_block(piece_block).await.context("sending to disk handle")?;
                        },
                        ClientMessage::Have { piece } => {
                            self.torrent_context.bitfield.lock().await[piece as usize / 8] |= 1 << (7 - piece % 8);  

                            for peer_handle in &mut self.peer_handles {
                                let _ = peer_handle.have(piece as u32).await;
                            }                          
                        },
                        ClientMessage::Request { piece_block, tx } => {
                            self.disk_handle.read_block(piece_block, tx).await.context("sending to disk handle")?;
                        },
                        ClientMessage::FinishedDownloading => {
                            Torrent::save_state(self.torrent_context.clone()).await.context("saving torrent state")?;
                            println!("Finished downloading torrent '{}'", self.torrent_context.torrent_name);

                            if let Some(ref mut tracker) = tracker {
                                if let Err(e) = self.tracker_completed(tracker).await {
                                    eprintln!("Failed to send completed message to tracker: {}", e);
                                }
                            } 
                        },
                        ClientMessage::SendTorrentInfo { tx } => {
                            let torrent_state = TorrentContextState::new(self.torrent_context.clone()).await;
                            if let Err(e) = tx.send(torrent_state) {
                                eprintln!("Failed to send torrent info for torrent '{}' to client: {:?}", self.torrent_context.torrent_name, e);
                            }
                        },
                        ClientMessage::PeerDisconnected { peer_address } => {
                            self.torrent_context.peers.retain(|peer| peer != &peer_address);

                            let handle_index = self.peer_handles.iter().position(|peer_handle| peer_handle.peer_address == peer_address);
                            if let Some(handle_index) = handle_index {
                                if let Err(e) = self.peer_handles.remove(handle_index).join().await {
                                    eprintln!("Failed to join peer handle when disconnecting peer: {:?}", e);
                                }
                            }
                        },
                        _ => {}
                    }
                },
                _ = tokio::time::sleep(std::time::Duration::from_secs(120)) => {
                    // connect to more peers with better tracker request
                    if self.torrent_context.needed.lock().await.needed_blocks.len() > 0 {
                        if let Some(ref mut tracker) = tracker {
                            // connect to peers from tracker response
                            if let Err(e) = self.connect_to_peers(tracker).await {
                                eprintln!("Failed to connect to peers: {}", e);
                            }
                        }
                    }
                }
                // TODO: listen to an open socket for seeding
                else => {
                    println!("Trying to listen for seeding");
                }
            }
        }

        for peer_handle in self.peer_handles {
            let peer_addr = peer_handle.peer_address.clone();
            if let Err(err) = peer_handle.join().await {
                if let Some(err) = err.downcast_ref::<tokio::io::Error>() {
                    match err.kind() {
                        tokio::io::ErrorKind::UnexpectedEof => {
                            // println!("Peer '{peer_addr}' closed the connection");
                            self.torrent_context.peers.retain(|peer| peer != &peer_addr);
                        }
                        tokio::io::ErrorKind::ConnectionReset => {
                            // println!("Peer '{peer_addr}' disconnected");
                            self.torrent_context.peers.retain(|peer| peer != &peer_addr);

                        }
                        tokio::io::ErrorKind::ConnectionAborted => {
                            // println!("Peer '{peer_addr}' was disconnected");
                            self.torrent_context.peers.retain(|peer| peer != &peer_addr);

                        }
                        _ => {
                            eprintln!("Peer '{peer_addr}' errored: {err:?}");
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