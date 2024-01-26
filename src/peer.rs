use futures::StreamExt;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use anyhow::{anyhow, Result};

use std::sync::Arc;

use crate::messager::ClientMessage;
use crate::torrent::{Sha1Hash, TorrentInfo};
use crate::utils::{AsBytes, CommunicationPipe};

pub mod peer_address;
pub use peer_address::PeerAddress;  

pub mod peer_message;
pub use peer_message::{PeerMessage, PeerSession, ConnectionType};

pub mod block_picker;
pub use block_picker::{BlockPicker, BlockPickerState};

use self::peer_message::Handshake;


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceBlock {
    pub block_index: usize,
    pub piece_index: usize,
    pub offset: usize,
    pub size: usize,
    pub data: Vec<u8>,
}

pub struct PeerTorrentContext {
    tx: mpsc::Sender<ClientMessage>,

    pub torrent_info: Arc<TorrentInfo>,
    pub info_hash: Sha1Hash,
    pub needed: Arc<Mutex<BlockPicker>>,
    pub bitfield: Arc<Mutex<Vec<u8>>>,

    pub downloaded: Arc<Mutex<u64>>,
    pub uploaded: Arc<Mutex<u64>>,
}

impl PeerTorrentContext {
    pub fn new(tx: mpsc::Sender<ClientMessage>, torrent_info: Arc<TorrentInfo>, info_hash: Sha1Hash, needed: Arc<Mutex<BlockPicker>>, bitfield: Arc<Mutex<Vec<u8>>>, uploaded: Arc<Mutex<u64>>, downloaded: Arc<Mutex<u64>>) -> Self {
        Self {
            tx,

            torrent_info,
            info_hash,
            needed,
            bitfield,

            downloaded,
            uploaded,
        }
    }
}

struct PeerContext {
    id: [u8;20],
    ip: PeerAddress,
    am_interested: bool,
    am_choking: bool,
    interested: bool,
    choking: bool,
    having_pieces: Vec<usize>,
}

pub struct PeerHandle {
    tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<Result<()>>,

    pub peer_address: PeerAddress,
}

impl PeerHandle {
    pub async fn new(client_id: [u8; 20], torrent_context: PeerTorrentContext, peer_address: PeerAddress, connection_type: ConnectionType, disk_tx: mpsc::Sender<ClientMessage>) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(crate::MAX_CHANNEL_SIZE);

        let self_pipe = CommunicationPipe {
            tx: sender.clone(),
            rx: receiver
        };

        let peer = Peer::new(client_id, torrent_context, peer_address.clone(), self_pipe, disk_tx).await;
        let join_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            peer.run(connection_type, None).await
        });

        Ok(Self {
            tx: sender,
            join_handle,
            peer_address,
        })
    }

    pub async fn from_session(client_id: [u8; 20], torrent_context: PeerTorrentContext, session: PeerSession, disk_tx: mpsc::Sender<ClientMessage>) -> Result<PeerHandle> {
        let peer_address = PeerAddress {
            address: session.stream.peer_addr()?.ip().to_string(),
            port: session.stream.peer_addr()?.port().to_string(),
        };

        let connection_type = session.connection_type.clone();

        let (sender, receiver) = mpsc::channel(crate::MAX_CHANNEL_SIZE);

        let self_pipe = CommunicationPipe {
            tx: sender.clone(),
            rx: receiver
        };

        let peer = Peer::new(client_id, torrent_context, peer_address.clone(), self_pipe, disk_tx).await;
        let join_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            peer.run(connection_type, Some(session)).await
        });

        Ok(Self {
            tx: sender,
            join_handle,
            peer_address,
        })
    }

    pub async fn join(self) -> Result<()> {
        self.join_handle.await?
    }

    pub async fn cancel(&mut self, index: u32, begin: u32, length: u32) -> Result<()> {
        self.tx.send(ClientMessage::Cancel{index, begin, length}).await?;
        Ok(())
    }

    pub async fn have(&mut self, piece: u32) -> Result<()> {
        self.tx.send(ClientMessage::Have{piece}).await?;
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.tx.send(ClientMessage::Shutdown).await?;
        Ok(())
    }
}

struct Peer {
    self_tx: mpsc::Sender<ClientMessage>,

    rx: mpsc::Receiver<ClientMessage>,
    peer_context: PeerContext,
    torrent_context: PeerTorrentContext,
    disk_tx: mpsc::Sender<ClientMessage>,

    client_id: [u8; 20],
}

impl std::fmt::Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.peer_context.ip)
    }
}

impl Peer {
    pub async fn new(client_id: [u8; 20], torrent_context: PeerTorrentContext, addr: PeerAddress, self_pipe: CommunicationPipe, disk_tx: mpsc::Sender<ClientMessage>) -> Self {
        let peer_context = PeerContext {
            id: [0; 20],
            ip: addr,
            am_interested: false,
            am_choking: true,
            interested: false,
            choking: true,
            having_pieces: Vec::new(),
        };

        Self {
            self_tx: self_pipe.tx,
            rx: self_pipe.rx,
            peer_context,
            torrent_context,
            disk_tx,

            client_id,
        }
    }

    async fn keep_alive(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        peer_session.send(PeerMessage::KeepAlive).await?;
        Ok(())
    }

    async fn handshake(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        let bitfield = self.torrent_context.bitfield.lock().await.clone();
        peer_session.handshake(self.torrent_context.info_hash.clone(), self.client_id.clone(), bitfield).await?;
       
        self.peer_context.id = peer_session.peer_handshake.peer_id;

        Ok(())
    }

    async fn interested(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        peer_session.interested().await?;
        self.peer_context.am_interested = true;
        //// println!("Interested in peer: '{self}'");
        Ok(())
    }

    async fn not_interested(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        //// println!("Not interested in peer: '{self}'");
        peer_session.not_interested().await?;
        self.peer_context.am_interested = false;
        Ok(())
    }

    async fn choke(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        peer_session.choke().await?;
        self.peer_context.am_choking = true;
        Ok(())
    }

    async fn unchoke(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        peer_session.unchoke().await?;
        self.peer_context.am_choking = false;
        Ok(())
    }

    async fn request(&mut self, peer_session: &mut PeerSession, downloading_blocks: &mut Vec<PieceBlock>, last_request_elapsed: &mut bool) -> Result<()> {
        // if piece is not initialized or if the piece is fully downloaded
        // if crate::BLOCK_REQUEST_COUNT >= self.torrent_context.needed_blocks.lock().await.len() {
        //     if !*last_request_elapsed {
        //         for block in self.torrent_context.needed_blocks.lock().await.iter() {
        //             let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);
        //             let piece_index = block / blocks_in_piece;
    
        //             let block_offset = (block % blocks_in_piece) * crate::BLOCK_SIZE;
    
        //             let block_size = Peer::get_block_size(&self.torrent_context, *block);
    
        //             downloading_blocks.push(PieceBlock {
        //                 block_index: *block,
        //                 piece_index,
        //                 offset: block_offset,
        //                 size: block_size,
        //                 data: Vec::new(),
        //             });
        //             peer_session.send(PeerMessage::Request(piece_index as u32, block_offset as u32, block_size as u32)).await?;
        //             println!("Requesting block {} from peer: '{self}' with piece index {}, offset {} and size {}", block, piece_index, block_offset, block_size);
        //         }

        //         *last_request_elapsed = true;
        //     }

        //     return Ok(());
        // }

        while downloading_blocks.len() < crate::BLOCK_REQUEST_COUNT {
            match self.torrent_context.needed.lock().await.pick_random(&self.peer_context.having_pieces).await? {
                Some(block_index) => {
                    let piece_index = block_index / self.torrent_context.torrent_info.blocks_in_piece;
                    let offset = (block_index % self.torrent_context.torrent_info.blocks_in_piece) * crate::BLOCK_SIZE;
                    let size = self.torrent_context.torrent_info.get_specific_block_length(block_index);

                    downloading_blocks.push(PieceBlock {
                        block_index,
                        piece_index,
                        offset,
                        size,
                        data: Vec::new(),
                    });

                    peer_session.send(PeerMessage::Request(piece_index as u32, offset as u32, size as u32)).await?;
                    println!("Requesting block {} from peer: '{self}' with piece index {}, offset {} and size {}", block_index, piece_index, offset, size);
                },
                None => {
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    async fn get_peer_session(&self, connection_type: ConnectionType) -> Result<PeerSession> {
        let stream = tokio::select! {
            stream = tokio::net::TcpStream::connect(format!("{}:{}", self.peer_context.ip.address, self.peer_context.ip.port)) => stream,
            _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => return Err(anyhow!("Failed to connect to peer '{self}'"))
        }?;

        let peer_session = PeerSession::new(stream, connection_type, Handshake::new(Sha1Hash([0; 20]), [0; 20])).await;

        Ok(peer_session)
    }

    pub async fn run(mut self, connection_type: ConnectionType, peer_session: Option<PeerSession>) -> Result<()> {
        let mut peer_session = match peer_session {
            Some(peer_session) => peer_session,
            None => self.get_peer_session(connection_type).await?,
        };
            
        println!("Connected to peer: '{self}'");

        // handshake with peer
        self.handshake(&mut peer_session).await?;

        let mut last_request_elapsed = false; 

        let mut downloading_blocks: Vec<PieceBlock> = Vec::new();
        let mut seeding_blocks: Vec<PieceBlock> = Vec::new();
        loop {
            tokio::select! {
                biased;
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            // dropping last not fully downloaded piece
                            for block in downloading_blocks {
                                let block_index = {
                                    block.piece_index * self.torrent_context.torrent_info.blocks_in_piece + block.offset.div_ceil(crate::BLOCK_SIZE)
                                };

                                let mut needed_guard = self.torrent_context.needed.lock().await;
                                if !needed_guard.needed_blocks.contains(&block_index) {
                                    needed_guard.needed_blocks.push(block_index);
                                }
                                if !needed_guard.needed_pieces.contains(&block.piece_index) {
                                    needed_guard.needed_pieces.push(block.piece_index);
                                }
                            }

                            break;
                        },
                        ClientMessage::RequestedBlock{piece_block} => {
                            if seeding_blocks.iter().any(|block| block.block_index == piece_block.block_index) {
                                seeding_blocks.retain(|block| block.block_index != piece_block.block_index);
                                
                                *self.torrent_context.uploaded.lock().await += piece_block.data.len() as u64;
                                let message = PeerMessage::Piece(piece_block.piece_index as u32, piece_block.offset as u32, piece_block.data.clone());
                                println!("Sending message: {:?}", message.as_bytes());
                                peer_session.send(PeerMessage::Piece(piece_block.piece_index as u32, piece_block.offset as u32, piece_block.data)).await?;
                            }
                        },
                        ClientMessage::Cancel{index, begin, length} => {
                            let block_index = {
                                index as usize * self.torrent_context.torrent_info.blocks_in_piece + begin.div_ceil(crate::BLOCK_SIZE as u32) as usize
                            };

                            let piece_block = PieceBlock {
                                block_index,
                                piece_index: index as usize,
                                offset: begin as usize,
                                size: length as usize,
                                data: Vec::new(),
                            };

                            if !downloading_blocks.contains(&piece_block) {
                                continue;
                            }

                            downloading_blocks.retain(|block| block != &piece_block);
                            println!("Canceling block {} from peer: '{self}' with piece index {}, offset {} and size {}", block_index, index, begin, length);
                            peer_session.send(PeerMessage::Cancel(index, begin, length)).await?;
                        },
                        ClientMessage::Have{piece} => {
                            if !self.peer_context.having_pieces.contains(&(piece as usize)) {
                                peer_session.send(PeerMessage::Have(piece as u32)).await?;
                            }
                        }
                        _ => {}
                    }
                }
                peer_message = peer_session.recv() => {
                    match peer_message? {
                        PeerMessage::Choke => {
                            self.peer_context.choking = true;
                        },
                        PeerMessage::Unchoke => {
                            self.peer_context.choking = false;
                        },
                        PeerMessage::Interested => {
                            self.peer_context.interested = true;
                            self.unchoke(&mut peer_session).await?;
                        },
                        PeerMessage::NotInterested => {
                            self.peer_context.interested = false;
                            self.choke(&mut peer_session).await?;
                        },
                        PeerMessage::Have(index) => {
                            if !self.peer_context.having_pieces.contains(&(index as usize)) {
                                self.peer_context.having_pieces.push(index as usize);
                            }
                        },
                        PeerMessage::Bitfield(bitfield) => {
                            let mut bitfield_iter = tokio_stream::iter(bitfield).enumerate();
                            
                            let mut piece_numbers = Vec::new();
                            while let Some((i, byte)) = bitfield_iter.next().await {
                                for j in 0..8 {
                                    if byte & (1 << (7 - j)) != 0 {
                                        piece_numbers.push(i * 8 + j);
                                    }
                                }
                            }

                            self.peer_context.having_pieces = piece_numbers;
                        },
                        PeerMessage::Request(index, begin, length) => {
                            if self.peer_context.am_choking || !self.peer_context.interested {
                                return Err(anyhow!("Peer '{self}' sent request when I am choking or they are not interested"));
                            }
                            
                            let block_index = {
                                index as usize * self.torrent_context.torrent_info.blocks_in_piece + begin.div_ceil(crate::BLOCK_SIZE as u32) as usize
                            };

                            let seeding_block = PieceBlock {
                                block_index: block_index,
                                piece_index: index as usize,
                                offset: begin as usize,
                                size: length as usize,
                                data: Vec::new(),
                            };

                            if seeding_blocks.contains(&seeding_block) {
                                return Err(anyhow!("Peer '{self}' sent request for block that is already being seeded"));
                            }

                            seeding_blocks.push(seeding_block.clone());

                            self.torrent_context.tx.send(ClientMessage::Request{piece_block: seeding_block, tx: self.self_tx.clone()}).await?;
                        },
                        PeerMessage::Piece(index, begin, block) => {
                            if !self.peer_context.am_interested || self.peer_context.choking {
                                return Err(anyhow!("Peer '{self}' sent piece block when I am not interested or they are choking"));
                            }

                            let block_index = {
                                index as usize * self.torrent_context.torrent_info.blocks_in_piece + begin.div_ceil(crate::BLOCK_SIZE as u32) as usize
                            };

                            let mut piece_block = PieceBlock {
                                block_index: block_index,
                                piece_index: index as usize,
                                offset: begin as usize,
                                size: block.len(),
                                data: Vec::new(),
                            };

                            if downloading_blocks.is_empty() {
                                return Err(anyhow!("Peer '{self}' received piece block when no piece block was requested"));
                            }

                            if !downloading_blocks.contains(&piece_block) {
                                return Err(anyhow!("Peer '{self}' sent wrong piece"));
                            }

                            downloading_blocks.retain(|block| block != &piece_block);
                            
                            piece_block.data = block;

                            // send it to the client and remove it from the downloading blocks
                            *self.torrent_context.downloaded.lock().await += piece_block.data.len() as u64;
                            self.disk_tx.send(ClientMessage::DownloadedBlock{ piece_block }).await?;
                        },
                        PeerMessage::Cancel(index, begin, length) => {
                            unimplemented!("Peer '{self}' sent cancel: {index}, {begin}, {length}")
                        },
                        PeerMessage::Port(port) => {
                            unimplemented!("Peer '{self}' sent port: {port}")
                        },
                        PeerMessage::KeepAlive => {
                            continue;
                        },
                        _ => {
                            return Err(anyhow!("Peer '{self}' sent invalid message"));
                        }
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(120)) => {
                    // 2 minutes without any message from peer elapsed, sending keep alive
                    self.keep_alive(&mut peer_session).await?;
                }
            }

            // send interested if there are pieces to download
            if !self.peer_context.am_interested && !self.torrent_context.needed.lock().await.needed_blocks.is_empty() {
                self.interested(&mut peer_session).await?;
            }

            // if interested in downloading and unchoked
            if !self.peer_context.choking && self.peer_context.am_interested {
                // if bitfield is empty then the peer has no pieces to download
                if self.peer_context.having_pieces.is_empty() {
                    continue;
                }

                if downloading_blocks.is_empty() {
                    if self.torrent_context.needed.lock().await.is_empty() {
                        self.not_interested(&mut peer_session).await?;
                    }
                    else {
                        self.request(&mut peer_session, &mut downloading_blocks, &mut last_request_elapsed).await?;
                    }
                }
            }

            // file is fully downloaded and peer doesn't want to download as well
            if !self.peer_context.am_interested && self.peer_context.having_pieces.len() == self.torrent_context.torrent_info.pieces_count {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                self.torrent_context.tx.send(ClientMessage::PeerDisconnected{peer_address: self.peer_context.ip}).await?;
                break;
            }
        }

        Ok(())  
    }
}
