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
    block_bitfield: Vec<usize>,
}

pub struct PeerTorrentContext {
    tx: mpsc::Sender<ClientMessage>,

    piece_length: usize,
    torrent_length: u64,
    pieces_count: usize,
    blocks_count: usize,
    info_hash: Sha1Hash,
    blocks: Arc<Mutex<Vec<usize>>>,

    uploaded: Arc<Mutex<u64>>,
}

impl PeerTorrentContext {
    pub fn new(tx: mpsc::Sender<ClientMessage>, piece_length: usize, torrent_length: u64, info_hash: Sha1Hash, blocks: Arc<Mutex<Vec<usize>>>, pieces_count: usize, blocks_count: usize, uploaded: Arc<Mutex<u64>>) -> Self {
        Self {
            tx,

            piece_length,
            torrent_length,
            pieces_count,
            blocks_count,
            info_hash,
            blocks,

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
            block_bitfield: Vec::new(),
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
        let mut block_guard = self.torrent_context.blocks.lock().await;

        let common_indexes = self.peer_context.block_bitfield
            .iter()
            .filter(|&i| block_guard.contains(i))
            .collect::<Vec<&usize>>();

        if common_indexes.is_empty() {
            return None;
        }

        let block_index = match rand_number_u32(0, common_indexes.len() as u32) {
            Ok(block_index) => block_index as usize,
            Err(_) => {
                eprintln!("Failed to generate random number");
                return None;
            }
        };

        let block = common_indexes[block_index];

        block_guard.retain(|&i| i != *block);

        Some(*block)
    }

    async fn keep_alive(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        peer_session.send(PeerMessage::KeepAlive).await?;
        Ok(())
    }

    async fn handshake(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        let bitfield = self.bitfield().await?;
        self.peer_context.id = peer_session.handshake(self.torrent_context.info_hash.clone(), self.client_id.clone(), bitfield.clone()).await?;
        if !bitfield.is_empty() {
            //// println!("Bitfield with peer '{self}': {bitfield:?}");
        }
        Ok(())
    }

    async fn contains_none(&self, piece_blocks: Vec<usize>) -> bool {
        let blocks_left = self.torrent_context.blocks.lock().await;
        for block in piece_blocks {
            if blocks_left.contains(&block) {
                return false;
            }
        }
        true
    }

    async fn bitfield(&self) -> Result<Vec<u8>> {
        let mut available_pieces = Vec::new();

        let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);
        let mut block_count = self.torrent_context.blocks_count;

        let mut i = 0;
        while block_count > blocks_in_piece {
            let piece_blocks = (i..i+blocks_in_piece).collect::<Vec<usize>>();

            if self.contains_none(piece_blocks).await {
                available_pieces.push(i / blocks_in_piece);
            }

            i += blocks_in_piece;
            block_count -= blocks_in_piece;
        }
        
        if block_count > 0 {
            let piece_blocks = (i..i+block_count).collect::<Vec<usize>>();

            if self.contains_none(piece_blocks).await {
                available_pieces.push(i / blocks_in_piece);
            }
        }


        if available_pieces.is_empty() {
            return Ok(Vec::new());
        }

        let bifield_size = self.torrent_context.pieces_count.div_ceil(8);
        let mut bitfield = vec![0; bifield_size];

        for piece in available_pieces {
            let byte = piece / 8;
            let bit = piece % 8;
            bitfield[byte] |= 1 << (7 - bit);
        }

        Ok(bitfield)
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

    async fn request(&mut self, peer_session: &mut PeerSession, downloading_blocks: &mut Vec<PieceBlock>) -> Result<()> {
        // if piece is not initialized or if the piece is fully downloaded
        for _ in 0..crate::BLOCK_REQUEST_COUNT {
            match self.get_random_block_index().await {
                Some(block_index) => {
                    let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);
                    let piece_index = block_index / blocks_in_piece;

                    let block_offset = (block_index % blocks_in_piece) * crate::BLOCK_SIZE;

                    let block_size = Peer::get_block_size(&self.torrent_context, block_index);

                    downloading_blocks.push(PieceBlock {
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
       // println!("Handshake with peer '{self}' successful");

        let mut downloading_blocks: Vec<PieceBlock> = Vec::new();
        loop {
            tokio::select! {
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            // dropping last not fully downloaded piece
                            for block in downloading_blocks {
                                let block_index = {
                                    let block_n = block.offset.div_ceil(crate::BLOCK_SIZE);
                                    let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);
                                    block.piece_index * blocks_in_piece + block_n
                                };
                                self.torrent_context.blocks.lock().await.push(block_index);
                            }

                            break;
                        },
                        _ => {}
                    }
                }
                peer_message = peer_session.recv() => {
                    match peer_message? {
                        PeerMessage::Choke => {
                           // println!("Peer '{self}' choke");
                            self.peer_context.choking = true;
                            todo!();
                        },
                        PeerMessage::Unchoke => {
                           // println!("Peer '{self}' unchoke");
                            self.peer_context.choking = false;
                        },
                        PeerMessage::Interested => {
                           // println!("Peer '{self}' interested");
                            self.peer_context.interested = true;
                            todo!();
                        },
                        PeerMessage::NotInterested => {
                           // println!("Peer '{self}' not interested");
                            self.peer_context.interested = false;
                            self.choke(&mut peer_session).await?;
                        },
                        PeerMessage::Have(index) => {
                           // println!("Peer '{self}' have {}", index);
                            // if !self.peer_context.bitfield.contains(&(index as usize)) {
                            //     self.peer_context.bitfield.push(index as usize);
                            // }
                            todo!();
                        },
                        PeerMessage::Bitfield(bitfield) => {
                           // println!("Peer '{self}' bitfield {:?}", bitfield);

                            let mut available_pieces = Vec::new();
                            for (i, byte) in bitfield.iter().enumerate() {
                                for j in 0..8 {
                                    if byte & (1 << (7 - j)) != 0 {
                                        available_pieces.push(i * 8 + j);
                                    }
                                }
                            }

                            self.peer_context.block_bitfield = {
                                let mut blocks_left = Vec::new();
                                let blocks_in_piece = self.torrent_context.piece_length.div_ceil(crate::BLOCK_SIZE);

                                for piece in available_pieces.iter() {
                                    let piece_size = Peer::get_piece_size(&self.torrent_context, *piece);
                                    let blocks_count = piece_size.div_ceil(crate::BLOCK_SIZE);

                                    for block in 0..blocks_count {
                                        let block_index = *piece * blocks_in_piece + block;
                                        blocks_left.push(block_index);
                                    }
                                }

                                blocks_left
                            };
                        },
                        PeerMessage::Request(index, begin, length) => {
                           // println!("Peer '{self}' request {} {} {}", index, begin, length);
                            todo!("send piece");

                            *self.torrent_context.uploaded.lock().await += length as u64;
                        },
                        PeerMessage::Piece(index, begin, block) => {
                            // println!("Peer '{self}' piece {} {}", index, begin);
                            let mut piece_block = PieceBlock {
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
                           // println!("Peer '{self}' cancel {} {} {}", index, begin, length);
                            todo!();
                        },
                        PeerMessage::Port(port) => {
                           // println!("Peer '{self}' port {}", port);
                            todo!();
                        },
                        PeerMessage::KeepAlive => {
                           // println!("Peer '{self}' keep alive");
                            // the counter should reset every time a message is received or a keep alive is
                            // sent from me
                            continue;
                            todo!("make a counter and disconnect if any messages aren't received for a long time");
                        },
                        _ => {
                            return Err(anyhow!("Peer '{self}' sent invalid message"));
                        }
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                    // TODO: send keep alive message every 60 secs (max 120)
                    // if !self.torrent_context.pieces.lock().await.is_empty() {
                    //    // println!("Sending keep alive to peer: '{self}'");
                    //     self.keep_alive(&mut peer_session).await?;
                    // }
                }
            }

            // send interested if there are pieces to download
            if !self.peer_context.am_interested && !self.torrent_context.blocks.lock().await.is_empty() {
                self.interested(&mut peer_session).await?;
            }

            // if interested in downloading and unchoked
            if !self.peer_context.choking && self.peer_context.am_interested {

                // if bitfield is empty then the peer has no pieces to download
                if self.peer_context.block_bitfield.is_empty() {
                    continue;
                }

                if downloading_blocks.is_empty() {
                    if self.torrent_context.blocks.lock().await.is_empty() {
                        self.not_interested(&mut peer_session).await?;
                    }
                    else {
                        self.request(&mut peer_session, &mut downloading_blocks).await?;
                    }
                }
            }

            // file is fully downloaded and peer doesn't want to download as well
            if !self.peer_context.am_interested && self.peer_context.block_bitfield.len() == self.torrent_context.pieces_count {
               // println!("File is fully downloaded and peer doesn't want to download as well");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                self.torrent_context.tx.send(ClientMessage::PeerDisconnected{peer_address: self.peer_context.ip}).await?;
                break;
            }
        }

        Ok(())  
    }
}
