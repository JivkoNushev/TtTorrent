use futures::StreamExt;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use anyhow::{anyhow, Result};

use std::sync::Arc;

use crate::messager::ClientMessage;
use crate::torrent::Sha1Hash;
use crate::utils::rand_number_u32;

pub mod peer_address;
pub use peer_address::PeerAddress;  

pub mod peer_message;
pub use peer_message::{PeerMessage, PeerSession, ConnectionType};

// TODO: change the block size based on the torrent file 

#[derive(Debug, PartialEq, Eq)]
pub struct PieceBlock {
    pub block_index: usize,
    pub piece_index: usize,
    pub offset: usize,
    pub size: usize,
    pub data: Vec<u8>,
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

pub struct PeerTorrentContext {
    tx: mpsc::Sender<ClientMessage>,

    piece_length: usize,
    torrent_length: u64,
    pieces_count: usize,
    blocks_count: usize,
    info_hash: Sha1Hash,
    needed_blocks: Arc<Mutex<Vec<usize>>>,
    needed_pieces: Arc<Mutex<Vec<usize>>>,
    bitfield: Arc<Mutex<Vec<u8>>>,

    uploaded: Arc<Mutex<u64>>,
}

impl PeerTorrentContext {
    pub fn new(tx: mpsc::Sender<ClientMessage>, piece_length: usize, torrent_length: u64, info_hash: Sha1Hash, needed_blocks: Arc<Mutex<Vec<usize>>>, needed_pieces: Arc<Mutex<Vec<usize>>>, bitfield: Arc<Mutex<Vec<u8>>>, pieces_count: usize, blocks_count: usize, uploaded: Arc<Mutex<u64>>) -> Self {
        Self {
            tx,

            piece_length,
            torrent_length,
            pieces_count,
            blocks_count,
            info_hash,
            needed_blocks,
            needed_pieces,
            bitfield,

            uploaded,
        }
    }
}

pub struct PeerHandle {
    tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<Result<()>>,

    pub peer_address: PeerAddress,
}

impl PeerHandle {
    pub async fn new(client_id: [u8; 20], torrent_context: PeerTorrentContext, peer_address: PeerAddress, connection_type: ConnectionType) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(crate::MAX_CHANNEL_SIZE);

        let peer = Peer::new(client_id, torrent_context, peer_address.clone(), receiver).await;
        let join_handle: JoinHandle<Result<()>>= tokio::spawn(async move {
            peer.run(connection_type).await
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
    pub async fn new(client_id: [u8; 20], torrent_context: PeerTorrentContext, addr: PeerAddress, rx: mpsc::Receiver<ClientMessage>, ) -> Self {
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
            rx,
            peer_context,
            torrent_context,

            client_id,
        }
    }

    fn get_piece_size(torrent_context: &PeerTorrentContext, piece_index: usize) -> usize {
        // TODO: make cleaner
        if piece_index == torrent_context.pieces_count - 1 {
            let size = (torrent_context.torrent_length % torrent_context.piece_length as u64) as usize;
            if 0 == size {
                torrent_context.piece_length
            }
            else {
                size
            }
        }
        else {
            torrent_context.piece_length
        }
    }

    fn get_block_size(torrent_context: &PeerTorrentContext, block_index: usize) -> usize {
        let blocks_in_piece = torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);
        let piece_index = block_index / blocks_in_piece;

        if piece_index != torrent_context.pieces_count - 1 {
            return crate::BLOCK_SIZE;
        }

        let piece_size = Peer::get_piece_size(torrent_context, piece_index);
        let blocks_in_piece = piece_size.div_ceil(crate::BLOCK_SIZE);
        let block_offset_index = block_index % blocks_in_piece;
        
        if block_offset_index != blocks_in_piece - 1 {
            return crate::BLOCK_SIZE;
        }
        
        let block_size = piece_size % crate::BLOCK_SIZE;
        if 0 == block_size{
            return crate::BLOCK_SIZE;
        }

        block_size
    }

    async fn get_random_block_index(&mut self) -> Option<usize> {
        let mut needed_pieces_guard = self.torrent_context.needed_pieces.lock().await;
        let mut needed_blocks_guard = self.torrent_context.needed_blocks.lock().await; 

        let common_pieces = self.peer_context.having_pieces
            .iter()
            .filter(|&i| needed_pieces_guard.contains(i))
            .collect::<Vec<&usize>>();

        if common_pieces.is_empty() {
            println!("peer {} has no common pieces", self.peer_context.ip);
            return None;
        }

        let random_piece_index = match rand_number_u32(0, common_pieces.len() as u32) {
            Ok(random_piece) => random_piece as usize,
            Err(_) => {
                eprintln!("Failed to generate random number");
                return None;
            }
        };

        let random_piece = common_pieces[random_piece_index];
        let piece_length = Peer::get_piece_size(&self.torrent_context, *random_piece);
        let blocks_in_current_piece = piece_length.div_ceil(crate::BLOCK_SIZE);
        let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);

        let piece_blocks = {

            let mut piece_blocks = Vec::new();

            let mut blocks_iter = tokio_stream::iter(0..blocks_in_current_piece);
            while let Some(i) = blocks_iter.next().await {
                let block = *random_piece * blocks_in_piece + i;
                if needed_blocks_guard.contains(&block) {
                    piece_blocks.push(block);
                }
            }

            piece_blocks
        };

        if piece_blocks.is_empty() {
            println!("peer {} has no blocks in piece {}", self.peer_context.ip, random_piece);
            return None;
        }

        let random_block_index = match rand_number_u32(0, piece_blocks.len() as u32) {
            Ok(random_block) => random_block as usize,
            Err(_) => {
                eprintln!("Failed to generate random number");
                return None;
            }
        };

        // if needed_blocks_guard.len() > crate::BLOCK_REQUEST_COUNT {
            needed_blocks_guard.retain(|&i| i != piece_blocks[random_block_index]);

            let blocks = (random_piece * blocks_in_piece..random_piece * blocks_in_piece + blocks_in_current_piece).collect::<Vec<usize>>();
            let retain_piece = blocks.iter().all(|&i| !needed_blocks_guard.contains(&i));
            if retain_piece {
                needed_pieces_guard.retain(|&i| i != *random_piece);
            }
        // }

        Some(piece_blocks[random_block_index])
    }

    async fn keep_alive(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        peer_session.send(PeerMessage::KeepAlive).await?;
        Ok(())
    }

    async fn handshake(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        let bitfield = self.torrent_context.bitfield.lock().await.clone();
        self.peer_context.id = peer_session.handshake(self.torrent_context.info_hash.clone(), self.client_id.clone(), bitfield).await?;
       
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

        for _ in 0..crate::BLOCK_REQUEST_COUNT {
            match self.get_random_block_index().await {
                Some(block_index) => {
                    let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);
                    let piece_index = block_index / blocks_in_piece;

                    let block_offset = (block_index % blocks_in_piece) * crate::BLOCK_SIZE;

                    let block_size = Peer::get_block_size(&self.torrent_context, block_index);

                    downloading_blocks.push(PieceBlock {
                        block_index,
                        piece_index,
                        offset: block_offset,
                        size: block_size,
                        data: Vec::new(),
                    });
                    peer_session.send(PeerMessage::Request(piece_index as u32, block_offset as u32, block_size as u32)).await?;
                    println!("Requesting block {} from peer: '{self}' with piece index {}, offset {} and size {}", block_index, piece_index, block_offset, block_size);
                },
                None => {
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    async fn get_peer_session(&self, connection_type: ConnectionType) -> Result<PeerSession> {
        let stream = match tokio::net::TcpStream::connect(format!("{}:{}", self.peer_context.ip.address, self.peer_context.ip.port)).await {
            Ok(stream) => stream,
            Err(e) => {
                return Err(anyhow!("Failed to connect to peer '{self}': {e}"));
            }
        };

        let peer_session = PeerSession::new(stream, connection_type).await;

        Ok(peer_session)
    }

    pub async fn run(mut self, connection_type: ConnectionType) -> Result<()> {
        let mut peer_session = self.get_peer_session(connection_type).await?;
        println!("Connected to peer: '{self}'");

        // handshake with peer
        self.handshake(&mut peer_session).await?;

        let mut last_request_elapsed = false; 

        let mut downloading_blocks: Vec<PieceBlock> = Vec::new();
        loop {
            tokio::select! {
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Cancel{index, begin, length} => {
                            let block_index = {
                                let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);
                                index as usize * blocks_in_piece + begin.div_ceil(crate::BLOCK_SIZE as u32) as usize
                            };

                            let piece_block = PieceBlock {
                                block_index: block_index,
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
                        ClientMessage::Shutdown => {
                            // dropping last not fully downloaded piece
                            for block in downloading_blocks {
                                let block_index = {
                                    let block_n = block.offset.div_ceil(crate::BLOCK_SIZE);
                                    let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);
                                    block.piece_index * blocks_in_piece + block_n
                                };
                                self.torrent_context.needed_blocks.lock().await.push(block_index);
                            }

                            break;
                        },
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
                                let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);
                                index as usize * blocks_in_piece + begin.div_ceil(crate::BLOCK_SIZE as u32) as usize
                            };

                            let seeding_block = PieceBlock {
                                block_index: block_index,
                                piece_index: index as usize,
                                offset: begin as usize,
                                size: length as usize,
                                data: Vec::new(),
                            };

                            unimplemented!("Peer '{self}' requested block: {seeding_block:?}")
                        },
                        PeerMessage::Piece(index, begin, block) => {
                            if !self.peer_context.am_interested || self.peer_context.choking {
                                return Err(anyhow!("Peer '{self}' sent piece block when I am not interested or they are choking"));
                            }

                            let block_index = {
                                let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);
                                index as usize * blocks_in_piece + begin.div_ceil(crate::BLOCK_SIZE as u32) as usize
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
                            self.torrent_context.tx.send(ClientMessage::DownloadedBlock{ block: piece_block }).await?;
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
            if !self.peer_context.am_interested && !self.torrent_context.needed_blocks.lock().await.is_empty() {
                self.interested(&mut peer_session).await?;
            }

            // if interested in downloading and unchoked
            if !self.peer_context.choking && self.peer_context.am_interested {
                // if bitfield is empty then the peer has no pieces to download
                if self.peer_context.having_pieces.is_empty() {
                    continue;
                }

                if downloading_blocks.is_empty() {
                    if self.torrent_context.needed_blocks.lock().await.is_empty() {
                        self.not_interested(&mut peer_session).await?;
                    }
                    else {
                        self.request(&mut peer_session, &mut downloading_blocks, &mut last_request_elapsed).await?;
                    }
                }
            }

            // file is fully downloaded and peer doesn't want to download as well
            if !self.peer_context.am_interested && self.peer_context.having_pieces.len() == self.torrent_context.pieces_count {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                self.torrent_context.tx.send(ClientMessage::PeerDisconnected{peer_address: self.peer_context.ip}).await?;
                break;
            }
        }

        Ok(())  
    }
}
