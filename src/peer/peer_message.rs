use anyhow::{anyhow, Result};
use tokio::io::{AsyncWriteExt, AsyncReadExt};

use crate::torrent::Sha1Hash;
use crate::utils::AsBytes;

#[derive(Debug)]
pub struct Handshake {
    protocol_len: u8,
    protocol: [u8; 19],
    reserved: [u8; 8],
    info_hash: [u8; 20],
    peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: Sha1Hash, peer_id: [u8; 20]) -> Self {
        Self {
            protocol_len: 19,
            protocol: *b"BitTorrent protocol",
            reserved: [0; 8],
            info_hash: info_hash.0,
            peer_id,
        }
    }

    fn from_peer_message(message: PeerMessage) -> Result<Self> {
        match message {
            PeerMessage::Handshake(handshake) => Ok(handshake),
            _ => Err(anyhow!("Invalid handshake")),
        }
    }
}

#[derive(Debug)]
pub enum PeerMessage {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request(u32, u32, u32),
    Piece(u32, u32, Vec<u8>),
    Cancel(u32, u32, u32),
    Port(u16),

    KeepAlive,
    Handshake(Handshake),
}

impl AsBytes for PeerMessage {
    fn as_bytes(&self) -> Vec<u8> {
        match self {
            PeerMessage::Choke => vec![0, 0, 0, 1, 0],
            PeerMessage::Unchoke => vec![0, 0, 0, 1, 1],
            PeerMessage::Interested => vec![0, 0, 0, 1, 2],
            PeerMessage::NotInterested => vec![0, 0, 0, 1, 3],
            PeerMessage::Have(index) => {
                let mut data = vec![0, 0, 0, 5, 4];
                data.extend_from_slice(&index.to_be_bytes());

                data
            },
            PeerMessage::Bitfield(bitfield) => {
                let mut data = vec![0, 0, 0, 1 + bitfield.len() as u8, 5];
                data.extend_from_slice(&bitfield);

                data
            },
            PeerMessage::Request(index, begin, length) => {
                let mut data = vec![0, 0, 0, 13, 6];
                data.extend_from_slice(&index.to_be_bytes());
                data.extend_from_slice(&begin.to_be_bytes());
                data.extend_from_slice(&length.to_be_bytes());

                data
            },
            PeerMessage::Piece(index, begin, block) => {
                let mut data = vec![0, 0, 0, 9 + block.len() as u8, 7];
                data.extend_from_slice(&index.to_be_bytes());
                data.extend_from_slice(&begin.to_be_bytes());
                data.extend_from_slice(&block);

                data
            },
            PeerMessage::Cancel(index, begin, length) => {
                let mut data = vec![0, 0, 0, 13, 8];
                data.extend_from_slice(&index.to_be_bytes());
                data.extend_from_slice(&begin.to_be_bytes());
                data.extend_from_slice(&length.to_be_bytes());

                data
            },
            PeerMessage::Port(port) => {
                let mut data = vec![0, 0, 0, 3, 9];
                data.extend_from_slice(&port.to_be_bytes());

                data
            },

            PeerMessage::KeepAlive => vec![0, 0, 0, 0],
            PeerMessage::Handshake(handshake) => {
                let mut data = Vec::new();
                data.push(handshake.protocol_len.clone());
                data.extend_from_slice(&handshake.protocol.clone());
                data.extend_from_slice(&handshake.reserved.clone());
                data.extend_from_slice(&handshake.info_hash.clone());
                data.extend_from_slice(&handshake.peer_id.clone());

                data
            },
        }
    }
}

impl PeerMessage {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let message = match bytes[0] {
            0 => Self::Choke,
            1 => Self::Unchoke,
            2 => Self::Interested,
            3 => Self::NotInterested,
            4 => Self::Have(u32::from_be_bytes(bytes[1..5].try_into().unwrap())),
            5 => Self::Bitfield(bytes[1..].to_vec()),
            6 => Self::Request(
                u32::from_be_bytes(bytes[1..5].try_into().unwrap()),
                u32::from_be_bytes(bytes[5..9].try_into().unwrap()),
                u32::from_be_bytes(bytes[9..13].try_into().unwrap()),
            ),
            7 => Self::Piece(
                u32::from_be_bytes(bytes[1..5].try_into().unwrap()),
                u32::from_be_bytes(bytes[5..9].try_into().unwrap()),
                bytes[9..].to_vec(),
            ),
            8 => Self::Cancel(
                u32::from_be_bytes(bytes[1..5].try_into().unwrap()),
                u32::from_be_bytes(bytes[5..9].try_into().unwrap()),
                u32::from_be_bytes(bytes[9..13].try_into().unwrap()),
            ),
            9 => Self::Port(u16::from_be_bytes(bytes[1..3].try_into().unwrap())),
            19 => Self::Handshake(Handshake {
                protocol_len: bytes[0],
                protocol: bytes[1..20].try_into().unwrap(),
                reserved: bytes[20..28].try_into().unwrap(),
                info_hash: bytes[28..48].try_into().unwrap(),
                peer_id: bytes[48..68].try_into().unwrap(),
            }),
            _ => return Err(anyhow::anyhow!("Invalid Peer message")),
        };
        Ok(message)
    } 

    pub fn new_handshake(info_hash: Sha1Hash, peer_id: [u8; 20]) -> Self {
        Self::Handshake(Handshake::new(info_hash, peer_id))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionType {
    Incoming,
    Outgoing,
}

pub struct PeerSession {
    stream: tokio::net::TcpStream,
    connection_type: ConnectionType,
}

impl PeerSession {
    pub async fn new(stream: tokio::net::TcpStream, connection_type: ConnectionType) -> Result<Self> {
        Ok(Self {
            stream,
            connection_type,
        })
    }

    async fn incoming_handshake(&mut self, info_hash: Sha1Hash) -> Result<Handshake> {
        let handshake = self.recv_handshake().await?;
        let handshake = Handshake::from_peer_message(handshake)?;
        if handshake.info_hash != info_hash.0 {
            return Err(anyhow!("Invalid info hash"));
        }

        Ok(handshake)
    }

    async fn outgoing_handshake(&mut self, info_hash: Sha1Hash, client_id: [u8; 20]) -> Result<()> {
        let handshake = PeerMessage::new_handshake(info_hash.clone(), client_id);
        self.send(handshake).await?;

        Ok(())
    }

    pub async fn handshake(&mut self, info_hash: Sha1Hash, client_id: [u8; 20], bitfield: Vec<u8>) -> Result<[u8; 20]> {
        let peer_id = match self.connection_type {
            ConnectionType::Outgoing => {
                self.outgoing_handshake(info_hash.clone(), client_id).await?;
                let incoming_handshake = self.incoming_handshake(info_hash).await?;

                incoming_handshake.peer_id
            }
            ConnectionType::Incoming => {
                let incoming_handshake = self.incoming_handshake(info_hash.clone()).await?;
                self.outgoing_handshake(info_hash, client_id).await?;

                incoming_handshake.peer_id
            }
        };

        // send bitfield
        if !bitfield.is_empty() {
            self.bitfield(bitfield).await?;
        }

        Ok(peer_id)
    }

    pub async fn bitfield(&mut self, bitfield: Vec<u8>) -> Result<()> {
        self.send(PeerMessage::Bitfield(bitfield)).await?;
        Ok(())
    }

    pub async fn interested(&mut self) -> Result<()> {
        self.send(PeerMessage::Interested).await?;
        Ok(())
    }

    pub async fn not_interested(&mut self) -> Result<()> {
        self.send(PeerMessage::NotInterested).await?;
        Ok(())
    }

    pub async fn choke(&mut self) -> Result<()> {
        self.send(PeerMessage::Choke).await?;
        Ok(())
    }

    pub async fn unchoke(&mut self) -> Result<()> {
        self.send(PeerMessage::Unchoke).await?;
        Ok(())
    }

    pub async fn send(&mut self, peer_message: PeerMessage) -> Result<()> {
        self.stream.write_all(&peer_message.as_bytes()).await?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<PeerMessage> {
        let mut message_size_bytes = [0; 4];
        self.stream.read_exact(&mut message_size_bytes).await?;

        let message_size = u32::from_be_bytes(message_size_bytes) as usize;
        if message_size == 0 {
            return Ok(PeerMessage::KeepAlive);
        }

        let mut message = vec![0; message_size];
        self.stream.read_exact(&mut message).await?;

        PeerMessage::from_bytes(&message)
    }

    pub async fn recv_handshake(&mut self) -> Result<PeerMessage> {
        let mut message = vec![0; 68];
        self.stream.read_exact(&mut message).await?;

        if message[0] != 19 {
            return Err(anyhow!("Invalid protocol length"));
        }

        if message[1..20] != *b"BitTorrent protocol" {
            return Err(anyhow!("Invalid protocol"));
        }

        PeerMessage::from_bytes(&message)
    }
}