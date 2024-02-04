use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use anyhow::{anyhow, Result};

use crate::messager::ClientMessage;
use crate::peer::block_picker::Piece;
use crate::utils::CommunicationPipe;
use crate::utils::sha1hash::Sha1Hash;

pub mod peer_address;
pub use peer_address::PeerAddress;  

pub mod peer_message;
pub use peer_message::{PeerMessage, PeerSession, ConnectionType, Handshake};

pub mod block_picker;
pub use block_picker::{BlockPicker, BlockPickerState, Block};

pub mod context;
pub use context::PeerTorrentContext;
use context::PeerContext;


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

    pub async fn cancel(&mut self, block: Block) -> Result<()> {
        self.tx.send(ClientMessage::Cancel{ block }).await?;
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
            bitfield: Vec::new(),
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
        tracing::debug!("Sending keep alive to peer: '{self}'");

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
        Ok(())
    }

    async fn not_interested(&mut self, peer_session: &mut PeerSession) -> Result<()> {
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

    async fn request(&mut self, peer_session: &mut PeerSession, downloading_blocks: &mut Vec<Block>, end_game_blocks: &mut Vec<Block>) -> Result<()> {
        if self.torrent_context.needed.lock().await.block_count() > crate::BLOCK_REQUEST_COUNT {
            while downloading_blocks.len() < crate::BLOCK_REQUEST_COUNT {
                match self.torrent_context.needed.lock().await.pick_random(&self.peer_context.bitfield).await? {
                    Some(block) => {
                        peer_session.send(PeerMessage::Request(block.index, block.begin, block.length)).await?;
                        tracing::debug!("Requested block {} from peer: '{self}' with piece index {}, offset {} and size {}", block.number, block.index, block.begin, block.length);
                        
                        downloading_blocks.push(block);
                    },
                    None => return Ok(())
                }
            }
        }
        else {
            if downloading_blocks.is_empty() {
                match self.torrent_context.needed.lock().await.get_end_game_blocks(&self.peer_context.bitfield).await? {
                    Some(blocks) => {
                        for block in blocks {
                            peer_session.send(PeerMessage::Request(block.index, block.begin, block.length)).await?;
                            tracing::debug!("Requested end game block {} from peer: '{self}' with piece index {}, offset {} and size {}", block.number, block.index, block.begin, block.length);
                            
                            if !end_game_blocks.iter().any(|b| b.number == block.number) {
                                end_game_blocks.push(block.clone());
                            }

                            downloading_blocks.push(block);
                        }
                    },
                    None => return Ok(())
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

    #[tracing::instrument(
        name = "Peer::run",
        skip(self),
        fields(
            peer_address = %self.peer_context.ip,
            connection_type = ?connection_type,
        )
    )]
    pub async fn run(mut self, connection_type: ConnectionType, peer_session: Option<PeerSession>) -> Result<()> {
        let mut peer_session = match peer_session {
            Some(peer_session) => peer_session,
            None => self.get_peer_session(connection_type).await?,
        };
        tracing::info!("Peer '{self}' connected");

        // handshake with peer
        self.handshake(&mut peer_session).await?;

        let mut end_game_blocks: Vec<Block> = Vec::new();
        let mut downloading_blocks: Vec<Block> = Vec::new();
        let mut seeding_blocks: Vec<Block> = Vec::new();
        loop {
            tracing::trace!("Peer '{self}' waiting for message");
            tokio::select! {
                biased;
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            for block in downloading_blocks {
                                let mut needed_guard = self.torrent_context.needed.lock().await;

                                if let Some(piece) = needed_guard.pieces.iter_mut().find(|piece| piece.index == block.index) {
                                    piece.block_count = self.torrent_context.torrent_info.blocks_in_piece;
                                }
                                else {
                                    needed_guard.pieces.push(Piece {
                                        index: block.index,
                                        block_count: self.torrent_context.torrent_info.blocks_in_piece,
                                    });
                                }
                            }
                            break;
                        },
                        ClientMessage::RequestedBlock{block} => {
                            let data = match block.data {
                                Some(data) => data,
                                None => {
                                    tracing::error!("Trying to send an empty block of data to peer {}", self.peer_context.ip);
                                    continue;
                                }
                            };

                            if data.len() as u32 != block.length {
                                tracing::error!("Trying to send a block of data with wrong length to peer {}", self.peer_context.ip);
                                continue;
                            }

                            if let Some(block_index) = seeding_blocks.iter().position(|b| b.number == block.number) {
                                seeding_blocks.remove(block_index);
                                
                                let length = block.length as u64;

                                peer_session.send(PeerMessage::Piece(block.index as u32, block.begin as u32, data)).await?;
                                *self.torrent_context.uploaded.lock().await += length;
                            }
                            else {
                                tracing::debug!("Block {} was canceled and not sending to peer {}", block.number, self.peer_context.ip);
                                continue;
                            }
                        },
                        ClientMessage::Have{piece} => {
                            tracing::trace!("Peer '{self}' received have message for piece {}", piece);
                            if !self.peer_context.bitfield.is_empty() &&  0 == self.peer_context.bitfield[piece as usize / 8] & 1 << (7 - piece % 8) {
                                peer_session.send(PeerMessage::Have(piece)).await?;
                            }
                        },
                        ClientMessage::Cancel{ block } => {
                            if let Some(block_index) = downloading_blocks.iter().position(|b| b.index == block.index && b.begin == block.begin && b.length == block.length) {
                                downloading_blocks.remove(block_index);
                                peer_session.send(PeerMessage::Cancel(block.index, block.begin, block.length)).await?;
                            }
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
                            self.peer_context.bitfield[index as usize / 8] |= 1 << (7 - index % 8);
                        },
                        PeerMessage::Bitfield(bitfield) => {
                            self.peer_context.bitfield = bitfield;
                        },
                        PeerMessage::Request(index, begin, length) => {
                            if  self.torrent_context.torrent_info.pieces_count as u32 <= index ||
                                begin + length > self.torrent_context.torrent_info.get_specific_piece_length(index) as u32 {
                                tracing::error!("Peer '{self}' sent an invalid request");
                                return Err(anyhow!("Peer '{self}' sent an invalid request"));
                            }

                            if self.peer_context.am_choking || !self.peer_context.interested {
                                tracing::error!("Peer '{self}' sent request when I am choking or they are not interested");
                                return Err(anyhow!("Peer '{self}' sent request when I am choking or they are not interested"));
                            }

                            if self.torrent_context.bitfield.lock().await[index as usize / 8] & 1 << (7 - index % 8) == 0 {
                                tracing::error!("Peer '{self}' sent request for piece that I don't have");
                                return Err(anyhow!("Peer '{self}' sent request for piece that I don't have"));
                            }

                            let block_index = {
                                index as usize * self.torrent_context.torrent_info.blocks_in_piece + begin.div_ceil(crate::BLOCK_SIZE as u32) as usize
                            };

                            let block = Block {
                                index,
                                begin,
                                length,

                                number: block_index,
                                data: None,
                            };

                            if seeding_blocks.iter().any(|b| b.number == block.number) {
                                tracing::error!("Peer '{self}' sent request for block that is already being seeded");
                                return Err(anyhow!("Peer '{self}' sent request for block that is already being seeded"));
                            }

                            seeding_blocks.push(block.clone());
                            self.disk_tx.send(ClientMessage::Request{block, tx: self.self_tx.clone()}).await?;

                        },
                        PeerMessage::Piece(index, begin, block) => {
                            if  self.torrent_context.torrent_info.pieces_count as u32 <= index ||
                                begin + block.len() as u32 > self.torrent_context.torrent_info.get_specific_piece_length(index) as u32 {
                                tracing::error!("Peer '{self}' sent an invalid request");
                                return Err(anyhow!("Peer '{self}' sent an invalid request"));
                            }
                            
                            if !self.peer_context.am_interested || self.peer_context.choking {
                                tracing::error!("Peer '{self}' sent piece block when I am not interested or they are choking");
                                return Err(anyhow!("Peer '{self}' sent piece block when I am not interested or they are choking"));
                            }

                            tracing::trace!("Peer '{self}' received block with index {}, begin {} and length {}", index, begin, block.len());

                            if downloading_blocks.iter().any(|b| b.index == index && b.begin == begin) {
                                let block_index = {
                                    index as usize * self.torrent_context.torrent_info.blocks_in_piece + begin.div_ceil(crate::BLOCK_SIZE as u32) as usize
                                };
                                let block = Block {
                                    index: index,
                                    begin: begin,
                                    length: block.len() as u32,
    
                                    number: block_index,
                                    data: Some(block),
                                };

                                tracing::trace!("Peer '{self}' retaining block: {}", block.number);
                                downloading_blocks.retain(|b| b.number != block_index);

                                if end_game_blocks.iter().any(|b| b.number == block.number) {
                                    tracing::trace!("Peer '{self}' received an end game block: {}", block.number);
                                    tracing::trace!("Peer '{self}' sending block to torrent task: {}", block.number);
                                    self.torrent_context.tx.send(ClientMessage::Cancel{ block }).await?;
                                }
                                else {
                                    tracing::trace!("Peer '{self}' sending block to disk task: {}", block.number);
                                    self.disk_tx.send(ClientMessage::DownloadedBlock{ block }).await?;
                                }
                            }
                        },
                        PeerMessage::Cancel(index, begin, length) => {
                            seeding_blocks.retain(|block| !(block.index == index && block.begin == begin && block.length == length));
                        },
                        PeerMessage::Port(port) => {
                            tracing::warn!("Peer '{self}' sent port: {port}, ignoring it");
                            continue;
                        },
                        PeerMessage::KeepAlive => {
                            tracing::debug!("Peer '{self}' sent keep alive");
                            continue;
                        },
                        _ => {
                            tracing::error!("Peer '{self}' sent invalid message");
                            return Err(anyhow!("Peer '{self}' sent invalid message"));
                        }
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(crate::CLIENT_KEEP_ALIVE_MESSAGE_INTERVAL_SECS)) => {
                    // 2 minutes without any message from peer elapsed, sending keep alive
                    self.keep_alive(&mut peer_session).await?;
                }
            }

            // send interested if there are pieces to download
            if !self.peer_context.am_interested && !self.torrent_context.needed.lock().await.is_empty() {
                self.interested(&mut peer_session).await?;
            }

            // if interested in downloading and unchoked
            if !self.peer_context.choking && self.peer_context.am_interested {
                if crate::utils::is_zero_aligned(&self.peer_context.bitfield) {
                    continue;
                }

                if self.torrent_context.needed.lock().await.is_empty() {
                    self.not_interested(&mut peer_session).await?;
                }
                else {
                    self.request(&mut peer_session, &mut downloading_blocks, &mut end_game_blocks).await?;
                }
            }

            // file is fully downloaded and peer doesn't want to download as well
            if !self.peer_context.am_interested && !self.peer_context.interested {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                tracing::info!("Peer '{self}' disconnecting");
                self.torrent_context.tx.send(ClientMessage::PeerDisconnected{peer_address: self.peer_context.ip}).await?;
                break;
            }
        }

        Ok(())  
    }
}
