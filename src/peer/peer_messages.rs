use tokio::{net::TcpStream, io::{AsyncWriteExt, AsyncReadExt}};

use crate::torrent::Sha1Hash;
use crate::utils::AsBytes;


pub mod handshake;
pub use handshake::Handshake;

#[derive(Debug, Clone, Copy)]
pub enum MessageID {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
    Port = 9,
}

impl MessageID {
    pub fn from(id: u8) -> MessageID {
        match id {
            0 => MessageID::Choke,
            1 => MessageID::Unchoke,
            2 => MessageID::Interested,
            3 => MessageID::NotInterested,
            4 => MessageID::Have,
            5 => MessageID::Bitfield,
            6 => MessageID::Request,
            7 => MessageID::Piece,
            8 => MessageID::Cancel,
            9 => MessageID::Port,
            _ => panic!("Invalid message id"),
        }
    }   
}

pub struct Message {
    pub size: u32,
    pub id: MessageID,
    pub payload: Vec<u8>,
}

impl AsBytes for Message {
    fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(&self.size.to_be_bytes());
        bytes.push(self.id as u8);
        bytes.extend_from_slice(&self.payload);

        bytes
    }
}

impl Message {
    pub fn new(id: MessageID, payload: Vec<u8>) -> Message {
        let size = payload.len() as u32 + 1;

        Message {
            size,
            id,
            payload,
        }
    }
}


// TODO: maybe use enum ?
pub struct PeerMessage {}

impl PeerMessage {
    pub async fn send_handshake(stream: &mut TcpStream, info_hash: &Sha1Hash, peer_id: &[u8; 20]) {
        let handshake = Handshake {
            protocol_len: 19,
            protocol: *b"BitTorrent protocol",
            reserved: [0; 8],
            info_hash: info_hash.as_bytes().clone(),
            peer_id: peer_id.clone(),
        };

        match stream.write_all(&handshake.as_bytes()).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't send handshake {}", e)
        }
    }

    pub async fn recv_handshake(stream: &mut TcpStream) -> Handshake {
        let mut buf = [0; 68];
        match stream.read_exact(&mut buf).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive handshake response {}", e)
        }

        Handshake::from_bytes(buf.to_vec())
    }

    pub async fn send_unchoke(stream: &mut TcpStream) {
        let unchoke = [0, 0, 0, 1, 1];

        match stream.write_all(&unchoke).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't send unchoke {}", e)
        }
    }

    pub async fn recv_unchoke(stream: &mut TcpStream) -> Vec<u8> {
        let mut recv_size: [u8; 4] = [0; 4];
        match stream.read_exact(&mut recv_size).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive unchoke size {}", e)
        }

        let recv_size = u32::from_be_bytes(recv_size);

        let mut buf: Vec<u8> = vec![0; recv_size as usize];

        match stream.read_exact(&mut buf).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive unchoke {}", e)
        }

        buf
    }

    pub async fn send_interested(stream: &mut TcpStream) {
        let interested = [0, 0, 0, 1, 2];

        match stream.write_all(&interested).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't send interested {}", e)
        }
    }

    pub async fn recv_interested(stream: &mut TcpStream) -> Vec<u8> {
        let mut recv_size: [u8; 4] = [0; 4];
        match stream.read_exact(&mut recv_size).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive interested size {}", e)
        }

        let recv_size = u32::from_be_bytes(recv_size);

        let mut buf: Vec<u8> = vec![0; recv_size as usize];

        match stream.read_exact(&mut buf).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive interested {}", e)
        }

        buf
    }

    pub async fn recv_bitfield(stream: &mut TcpStream) -> Vec<u8> {
        let mut recv_size: [u8; 4] = [0; 4];
        match stream.read_exact(&mut recv_size).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive bitfield size {}", e)
        }

        let recv_size = u32::from_be_bytes(recv_size);

        let mut buf: Vec<u8> = vec![0; recv_size as usize];

        match stream.read_exact(&mut buf).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive bitfield {}", e)
        }

        buf
    }

    pub async fn send_request(stream: &mut TcpStream, piece_index: u32, offset: u32, block_size: u32) {
        let mut interested = vec![0, 0, 0, 13, 6];

        interested.append(&mut piece_index.to_be_bytes().to_vec());
        interested.append(&mut offset.to_be_bytes().to_vec());
        interested.append(&mut block_size.to_be_bytes().to_vec());

        match stream.write_all(&interested).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't send request {}", e)
        }
    }

    pub async fn recv_piece(stream: &mut TcpStream) -> Vec<u8> {
        let mut recv_size: [u8; 4] = [0; 4];
        match stream.read_exact(&mut recv_size).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive piece size {}", e)
        }

        let recv_size = u32::from_be_bytes(recv_size);

        let mut buf: Vec<u8> = vec![0; recv_size as usize];

        match stream.read_exact(&mut buf).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive piece {}", e)
        }

        buf
    }
    
}