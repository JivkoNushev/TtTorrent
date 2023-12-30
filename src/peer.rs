use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use anyhow::Result;

use std::sync::Arc;

use crate::messager::ClientMessage;
use crate::torrent::Sha1Hash;
use crate::torrent::torrent_parser::TorrentParser;
use crate::torrent::torrent_file::BencodedValue;    
use crate::utils::rand_number_u32;

pub mod peer_address;
pub use peer_address::PeerAddress;  

pub mod peer_message;
pub use peer_message::{PeerMessage, PeerSession, ConnectionType};

// TODO: change the block size based on the torrent file 
const BLOCK_SIZE: usize = 1 << 14;

struct PeerContext {
    id: [u8;20],
    ip: PeerAddress,
    am_interested: bool,
    choking: bool,
    bitfield: Vec<usize>,
}

pub struct PeerTorrentContext {
    tx: mpsc::Sender<ClientMessage>,

    piece_length: usize,
    torrent_length: u64,
    info_hash: Sha1Hash,
    pieces: Arc<Mutex<Vec<usize>>>,
}

impl PeerTorrentContext {
    pub fn new(tx: mpsc::Sender<ClientMessage>, piece_length: usize, torrent_length: u64, info_hash: Sha1Hash, pieces: Arc<Mutex<Vec<usize>>>) -> Self {
        Self {
            tx,

            piece_length,
            torrent_length,
            info_hash,
            pieces,
        }
    }
}

struct Peer {
    rx: mpsc::Receiver<ClientMessage>,
    peer_context: PeerContext,
    torrent_context: PeerTorrentContext,

    client_id: [u8; 20],
}

impl std::fmt::Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.peer_context.ip)
    }
}

impl Peer {
    pub fn new(client_id: [u8; 20], torrent_context: PeerTorrentContext, addr: PeerAddress, rx: mpsc::Receiver<ClientMessage>, ) -> Self {
        let peer_context = PeerContext {
            id: [0; 20],
            ip: addr,
            am_interested: false,
            choking: true,
            bitfield: Vec::new(),
        };

        Self {
            rx,
            peer_context,
            torrent_context,

            client_id,
        }
    }

    fn get_piece_size(&self, piece_index: usize) -> usize {
        let pieces_count = self.torrent_context.torrent_length.div_ceil(self.torrent_context.piece_length as u64) as usize;

        if piece_index == pieces_count - 1 {
            (self.torrent_context.torrent_length % self.torrent_context.piece_length as u64) as usize
        }
        else {
            self.torrent_context.piece_length
        }
    }

    async fn get_random_piece_index(&mut self) -> Option<usize> {
        let mut pieces_guard = self.torrent_context.pieces.lock().await;

        let common_indexes = self.peer_context.bitfield
            .iter()
            .filter(|&i| pieces_guard.contains(i))
            .collect::<Vec<&usize>>();

        if common_indexes.is_empty() {
            return None;
        }

        let piece_index = rand_number_u32(0, common_indexes.len() as u32) as usize;

        let piece = common_indexes[piece_index];

        pieces_guard.retain(|&i| i != *piece);

        Some(*piece)
    }

    pub async fn run(mut self, connection_type: ConnectionType) -> Result<()> {
        let mut peer_session = PeerSession::new(connection_type, self.peer_context.ip.address.clone(), self.peer_context.ip.port.clone()).await?;
        println!("Peer {self} running");

        // handshake with peer
        self.peer_context.id = peer_session.handshake(self.torrent_context.info_hash.clone(), self.client_id.clone()).await?;

        // send interested message if outgoing connection
        self.peer_context.am_interested = peer_session.interest().await?;

        let mut current_piece_index = 0;
        let mut current_piece_size = 0;
        let mut current_piece_offset = 0;
        let mut current_block_count = 0;
        let mut current_block_size = 0;

        let mut current_piece = Vec::new();
        loop {
            println!("Trying to select");

            tokio::select! {
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            println!("Peer {self} stopping");

                            // TODO: is it better to wait for the piece to be downloaded or just drop it? (maybe dropping it is better)
                            if 0 != current_piece_size {
                                self.torrent_context.pieces.lock().await.push(current_piece_index);
                            }
                            break;
                        },
                        _ => {}
                    }
                }
                msg = peer_session.recv() => {
                    if let Err(e) = msg {
                        println!("Peer {self} error: {:?}", e);
                        break;
                    }
                    else if let Ok(msg) = msg {
                        match msg {
                            PeerMessage::Choke => {
                                println!("Peer {self} choke");
                                self.peer_context.choking = true;
                            },
                            PeerMessage::Unchoke => {
                                println!("Peer {self} unchoke");
                                self.peer_context.choking = false;
                            },
                            PeerMessage::Interested => {
                                println!("Peer {self} interested");
                                self.peer_context.am_interested = true;
                            },
                            PeerMessage::NotInterested => {
                                println!("Peer {self} not interested");
                                self.peer_context.am_interested = false;
                            },
                            PeerMessage::Have(index) => {
                                println!("Peer {self} have {}", index);
                                todo!();
                            },
                            PeerMessage::Bitfield(bitfield) => {
                                println!("Peer {self} bitfield {:?}", bitfield);

                                let mut available_pieces = Vec::new();
                                for (i, byte) in bitfield.iter().enumerate() {
                                    for j in 0..8 {
                                        if byte & (1 << (7 - j)) != 0 {
                                            available_pieces.push(i * 8 + j);
                                        }
                                    }
                                }

                                self.peer_context.bitfield = available_pieces;
                            },
                            PeerMessage::Request(index, begin, length) => {
                                println!("Peer {self} request {} {} {}", index, begin, length);
                                todo!();
                            },
                            PeerMessage::Piece(index, begin, block) => {
                                println!("Peer {self} piece {} {}", index, begin);

                                if 0 == current_piece_size {
                                    println!("Peer {self} received piece when no piece was requested");
                                    todo!();
                                }

                                if index as usize != current_piece_index || begin as usize != current_piece_offset - current_block_size {
                                    println!("Peer {self} sent wrong piece");
                                    todo!();
                                }
                                current_piece.extend(block);
                            },
                            PeerMessage::Cancel(index, begin, length) => {
                                println!("Peer {self} cancel {} {} {}", index, begin, length);
                                todo!();
                            },
                            PeerMessage::Port(port) => {
                                println!("Peer {self} port {}", port);
                                todo!();
                            },
                            _ => {
                                println!("Peer {self} sent unknown message: {msg:?}");
                            }
                        }
                    }
                }
            }

            if !self.peer_context.am_interested {
                todo!();
            }

            if self.peer_context.choking {
                continue;
            }

            // make requests for pieces
            if self.peer_context.bitfield.is_empty() {
                todo!();
            }
            
            if 0 != current_piece_size && current_piece_offset == current_piece_size {
                self.torrent_context.tx.send(ClientMessage::DownloadedPiece{piece_index: current_piece_index, piece: current_piece.clone()}).await?;
                current_piece_size = 0;
            }

            if 0 == current_piece_size {
                match self.get_random_piece_index().await {
                    Some(piece_index) => {
                        current_piece_index = piece_index;
                        current_piece_size = self.get_piece_size(piece_index);
                        current_piece_offset = 0;
                        current_block_count = 0;
                        current_block_size = BLOCK_SIZE;

                        current_piece.clear();
                    },
                    None => {
                        println!("Peer {self} has no more pieces to download right now");
                        if !self.torrent_context.pieces.lock().await.is_empty() {
                            todo!();
                        }

                        self.torrent_context.tx.send(ClientMessage::FinishedDownloading).await?;
    
                        break;
                    }
                }
            }
            
            if (current_block_count + 1) * BLOCK_SIZE > current_piece_size {
                current_block_size = current_piece_size % BLOCK_SIZE;
            }
            println!("Piece size: {}", current_piece_size);
            println!("Block size: {}", current_block_size);

            peer_session.send(PeerMessage::Request(current_piece_index as u32, current_piece_offset as u32, current_block_size as u32)).await?;
            println!("Peer {self} request {} {} {}", current_piece_index, current_piece_offset, current_block_size);
            
            current_piece_offset += current_block_size;
            current_block_count += 1;
        }

        Ok(())  
    }
}

pub struct PeerHandle {
    tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<()>,

    pub peer_address: PeerAddress,
}

impl PeerHandle {
    pub fn new(client_id: [u8; 20], torrent_context: PeerTorrentContext, addr: PeerAddress, connection_type: ConnectionType) -> Self {
        let (sender, receiver) = mpsc::channel(100);
        
        let peer = Peer::new(client_id, torrent_context, addr.clone(), receiver);
        let join_handle = tokio::spawn(async move {
            if let Err(e) = peer.run(connection_type).await {
                println!("Peer error: {:?}", e);
            }
        });

        Self {
            tx: sender,
            join_handle,
            peer_address: addr,
        }
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