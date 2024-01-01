use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use anyhow::{Result, anyhow, Error};

use std::sync::Arc;

use crate::messager::ClientMessage;
use crate::torrent::Sha1Hash;
use crate::utils::rand_number_u32;

pub mod peer_address;
pub use peer_address::PeerAddress;  

pub mod peer_message;
pub use peer_message::{PeerMessage, PeerSession, ConnectionType};

// TODO: change the block size based on the torrent file 
const BLOCK_SIZE: usize = 1 << 14;

struct PeerPiece {
    reseived: bool,

    index: usize,
    size: usize,
    offset: usize,
    block_count: usize,
    block_size: usize,

    data: Vec<u8>,
}

impl PeerPiece {
    fn new(index: usize, size: usize) -> Self {
        Self {
            reseived: false,

            index,
            size,
            offset: 0,
            block_count: 0,
            block_size: BLOCK_SIZE,

            data: Vec::new(),
        }
    }

    fn default() -> Self {
        Self {
            reseived: false,

            index: 0,
            size: 0,
            offset: 0,
            block_count: 0,
            block_size: BLOCK_SIZE,

            data: Vec::new(),
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
    bitfield: Vec<usize>,
}

pub struct PeerTorrentContext {
    tx: mpsc::Sender<ClientMessage>,

    piece_length: usize,
    torrent_length: u64,
    pieces_count: usize,
    info_hash: Sha1Hash,
    pieces: Arc<Mutex<Vec<usize>>>,

    // TODO: use arc mutex or send a message to the client ?
    uploaded: Arc<Mutex<u64>>,
}

impl PeerTorrentContext {
    pub fn new(tx: mpsc::Sender<ClientMessage>, piece_length: usize, torrent_length: u64, info_hash: Sha1Hash, pieces: Arc<Mutex<Vec<usize>>>, uploaded: Arc<Mutex<u64>>) -> Self {
        let pieces_count = torrent_length.div_ceil(piece_length as u64) as usize;
        Self {
            tx,

            piece_length,
            torrent_length,
            pieces_count,
            info_hash,
            pieces,

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
    pub async fn new(client_id: [u8; 20], torrent_context: PeerTorrentContext, stream: tokio::net::TcpStream, connection_type: ConnectionType) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(crate::MAX_CHANNEL_SIZE);

        let addr = stream.peer_addr()?;
        let peer_address = PeerAddress{
            address: addr.ip().to_string(),
            port: addr.port().to_string(),
        };

        let peer = Peer::new(client_id, torrent_context, peer_address.clone(), receiver);

        let peer_session = PeerSession::new(stream, connection_type).await?;
        let join_handle: JoinHandle<Result<()>>= tokio::spawn(async move {
            peer.run(peer_session).await
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
    pub fn new(client_id: [u8; 20], torrent_context: PeerTorrentContext, addr: PeerAddress, rx: mpsc::Receiver<ClientMessage>, ) -> Self {
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
            rx,
            peer_context,
            torrent_context,

            client_id,
        }
    }

    fn get_piece_size(&self, piece_index: usize) -> usize {
        // TODO: make cleaner
        if piece_index == self.torrent_context.pieces_count - 1 {
            let size = (self.torrent_context.torrent_length % self.torrent_context.piece_length as u64) as usize;
            if 0 == size {
                self.torrent_context.piece_length
            }
            else {
                size
            }
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

    async fn keep_alive(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        peer_session.send(PeerMessage::KeepAlive).await?;
        Ok(())
    }

    async fn handshake(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        let bitfield = self.bitfield().await?;
        self.peer_context.id = peer_session.handshake(self.torrent_context.info_hash.clone(), self.client_id.clone(), bitfield.clone()).await?;
        if !bitfield.is_empty() {
            println!("Bitfield with peer '{self}': {bitfield:?}");
        }
        Ok(())
    }

    async fn bitfield(&self) -> Result<Vec<u8>> {
        let mut available_pieces = (0..self.torrent_context.pieces_count).collect::<Vec<usize>>();
        {
            let pieces_guard = self.torrent_context.pieces.lock().await;
            available_pieces.retain(|&i| !pieces_guard.contains(&i));
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
        println!("Interested in peer: '{self}'");
        Ok(())
    }

    async fn not_interested(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        println!("Not interested in peer: '{self}'");
        peer_session.not_interested().await?;
        self.peer_context.am_interested = false;
        Ok(())
    }

    async fn choke(&mut self, peer_session: &mut PeerSession) -> Result<()> {
        peer_session.choke().await?;
        self.peer_context.am_choking = true;
        Ok(())
    }

    async fn request(&mut self, peer_session: &mut PeerSession, piece: &mut PeerPiece) -> Result<()> {
        // if the piece is initialized
        if 0 != piece.size {
            // if piece is fully downloaded send it to the client
            if !piece.reseived {
                return Ok(());
            }

            // if the piece is requested but not resived yet
            if piece.offset == piece.size {
                self.torrent_context.tx.send(ClientMessage::DownloadedPiece{piece_index: piece.index, piece: piece.data.clone()}).await?;
                piece.size = 0;
            }
        }
        
        // if piece is not initialized or if the piece is fully downloaded
        if 0 == piece.size {
            match self.get_random_piece_index().await {
                Some(piece_index) => {
                    *piece = PeerPiece::new(piece_index, self.get_piece_size(piece_index));
                },
                None => {
                    if self.torrent_context.pieces.lock().await.is_empty() {
                        self.not_interested(peer_session).await?;
                    }
                    return Ok(());
                }
            }
        }

        // last block of the piece might be smaller than BLOCK_SIZE
        if (piece.block_count + 1) * BLOCK_SIZE > piece.size {
            piece.block_size = piece.size % BLOCK_SIZE;
        }

        peer_session.send(PeerMessage::Request(piece.index as u32, piece.offset as u32, piece.block_size as u32)).await?;
        println!("Requesting piece {} {} {} from peer: '{self}'", piece.index, piece.offset, piece.block_size);

        piece.offset += piece.block_size;
        piece.block_count += 1;

        Ok(())
    }

    pub async fn run(mut self, mut peer_session: PeerSession) -> Result<()> {
        println!("Connected to peer: '{self}'");

        // handshake with peer
        self.handshake(&mut peer_session).await?;
        println!("Handshake with peer '{self}' successful");
        
        let mut downloading_piece = PeerPiece::default();
        loop {
            tokio::select! {
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            println!("Shutdown Peer '{self}'");

                            // dropping last not fully downloaded piece
                            if 0 != downloading_piece.size {
                                self.torrent_context.pieces.lock().await.push(downloading_piece.index);
                            }
                            break;
                        },
                        _ => {}
                    }
                }
                peer_message = peer_session.recv() => {
                    match peer_message? {
                        PeerMessage::Choke => {
                            println!("Peer '{self}' choke");
                            self.peer_context.choking = true;
                            todo!();
                        },
                        PeerMessage::Unchoke => {
                            println!("Peer '{self}' unchoke");
                            self.peer_context.choking = false;
                        },
                        PeerMessage::Interested => {
                            println!("Peer '{self}' interested");
                            self.peer_context.interested = true;
                            todo!();
                        },
                        PeerMessage::NotInterested => {
                            println!("Peer '{self}' not interested");
                            self.peer_context.interested = false;
                            self.choke(&mut peer_session).await?;
                        },
                        PeerMessage::Have(index) => {
                            println!("Peer '{self}' have {}", index);
                            if !self.peer_context.bitfield.contains(&(index as usize)) {
                                self.peer_context.bitfield.push(index as usize);
                            }
                        },
                        PeerMessage::Bitfield(bitfield) => {
                            println!("Peer '{self}' bitfield {:?}", bitfield);

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
                            println!("Peer '{self}' request {} {} {}", index, begin, length);
                            todo!("send piece");

                            *self.torrent_context.uploaded.lock().await += length as u64;
                        },
                        PeerMessage::Piece(index, begin, block) => {
                            println!("Peer '{self}' piece {} {}", index, begin);

                            if 0 == downloading_piece.size {
                                return Err(anyhow!("Peer '{self}' received piece when no piece was requested"));
                            }
                            if index as usize != downloading_piece.index || begin as usize != downloading_piece.offset - downloading_piece.block_size {
                                return Err(anyhow!("Peer '{self}' sent wrong piece"));
                            }

                            downloading_piece.data.extend(block);
                            downloading_piece.reseived = true;
                        },
                        PeerMessage::Cancel(index, begin, length) => {
                            println!("Peer '{self}' cancel {} {} {}", index, begin, length);
                            todo!();
                        },
                        PeerMessage::Port(port) => {
                            println!("Peer '{self}' port {}", port);
                            todo!();
                        },
                        PeerMessage::KeepAlive => {
                            println!("Peer '{self}' keep alive");
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
                    //     println!("Sending keep alive to peer: '{self}'");
                    //     self.keep_alive(&mut peer_session).await?;
                    // }
                }
            }

            // send interested if there are pieces to download
            if !self.peer_context.interested && !self.torrent_context.pieces.lock().await.is_empty() {
                self.interested(&mut peer_session).await?;
            }

            // if interested in downloading and unchoked
            if !self.peer_context.choking && self.peer_context.am_interested {
                // if bitfield is empty then the peer has no pieces to download
                if self.peer_context.bitfield.is_empty() {
                    continue;
                }

                self.request(&mut peer_session, &mut downloading_piece).await?;
            }

            // file is fully downloaded and peer doesn't want to download as well
            if !self.peer_context.am_interested && self.peer_context.bitfield.len() == self.torrent_context.pieces_count {
                println!("File is fully downloaded and peer doesn't want to download as well");
                // throwing an error so that the peer is removed from torrent peer
                // TODO: find a better way to do this
                return Err(Error::from(tokio::io::Error::new(tokio::io::ErrorKind::ConnectionAborted, "Connection aborted by client")));
            }
        }

        Ok(())  
    }
}
