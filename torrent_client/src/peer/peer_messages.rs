use tokio::{net::TcpStream, io::{AsyncWriteExt, AsyncReadExt}};

use crate::torrent::Sha1Hash;
use crate::utils::AsBytes;

#[derive(Debug)]
pub struct Handshake {
    pub protocol_len: u8,
    pub protocol: [u8; 19],
    pub reserved: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(hadnshake_bytes: Vec<u8>) -> Handshake{
        Handshake {
            protocol_len: hadnshake_bytes[0],
            protocol: hadnshake_bytes[1..20].try_into().unwrap(),
            reserved: hadnshake_bytes[20..28].try_into().unwrap(),
            info_hash: hadnshake_bytes[28..48].try_into().unwrap(),
            peer_id: hadnshake_bytes[48..68].try_into().unwrap(),
        }
    }
}

impl AsBytes for Handshake {
    fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.push(self.protocol_len);
        bytes.extend_from_slice(&self.protocol);
        bytes.extend_from_slice(&self.reserved);
        bytes.extend_from_slice(&self.info_hash);
        bytes.extend_from_slice(&self.peer_id);

        bytes
    }
}

pub struct Interested {

}

// TODO: maybe use enum ?
pub struct PeerMessage {}

impl PeerMessage {
    pub async fn send_handshake(stream: &mut TcpStream, info_hash: &Sha1Hash, peer_id: &[u8; 20]) {
        let handshake = Handshake {
            protocol_len: 19,
            protocol: *b"BitTorrent protocol",
            reserved: [0; 8],
            info_hash: info_hash.get_hash_ref().clone(),
            peer_id: peer_id.clone(),
        };

        stream.write_all(&handshake.as_bytes()).await.unwrap();
    }

    pub async fn recv_handshake(stream: &mut TcpStream) -> Handshake {
        let mut buf = [0; 68];
        stream.read_exact(&mut buf).await.unwrap();

        Handshake::new(buf.to_vec())
    }

    pub async fn send_unchoke(stream: &mut TcpStream) {
        let unchoke = [0, 0, 0, 1, 1];

        stream.write_all(&unchoke).await.unwrap();
    }

    pub async fn recv_unchoke(stream: &mut TcpStream) -> Vec<u8> {
        let mut recv_size: [u8; 4] = [0; 4];
        stream.read_exact(&mut recv_size).await.unwrap();

        let recv_size = u32::from_be_bytes(recv_size);

        let mut buf: Vec<u8> = vec![0; recv_size as usize];

        stream.read_exact(&mut buf).await.unwrap();

        buf
    }

    pub async fn send_interested(stream: &mut TcpStream) {
        let interested = [0, 0, 0, 1, 2];

        stream.write_all(&interested).await.unwrap();
    }

    pub async fn recv_interested(stream: &mut TcpStream) -> Vec<u8> {
        let mut recv_size: [u8; 4] = [0; 4];
        stream.read_exact(&mut recv_size).await.unwrap();

        let recv_size = u32::from_be_bytes(recv_size);

        let mut buf: Vec<u8> = vec![0; recv_size as usize];

        stream.read_exact(&mut buf).await.unwrap();

        buf
    }

    pub async fn recv_bitfield(stream: &mut TcpStream) -> Vec<u8> {
        let mut recv_size: [u8; 4] = [0; 4];
        stream.read_exact(&mut recv_size).await.unwrap();

        let recv_size = u32::from_be_bytes(recv_size);

        let mut buf: Vec<u8> = vec![0; recv_size as usize];

        stream.read_exact(&mut buf).await.unwrap();

        buf
    }

    pub async fn send_request(stream: &mut TcpStream, piece_index: u32, offset: u32, block_size: u32) {
        let mut interested = vec![0, 0, 0, 13, 6];

        interested.append(&mut piece_index.to_be_bytes().to_vec());
        interested.append(&mut offset.to_be_bytes().to_vec());
        interested.append(&mut block_size.to_be_bytes().to_vec());

        stream.write_all(&interested).await.unwrap();
    }

    pub async fn recv_piece(stream: &mut TcpStream) -> Vec<u8> {
        let mut recv_size: [u8; 4] = [0; 4];
        stream.read_exact(&mut recv_size).await.unwrap();

        let recv_size = u32::from_be_bytes(recv_size);

        let mut buf: Vec<u8> = vec![0; recv_size as usize];

        stream.read_exact(&mut buf).await.unwrap();

        buf
    }
    
}