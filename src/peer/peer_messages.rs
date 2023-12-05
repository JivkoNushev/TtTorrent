use tokio::{net::TcpStream, io::{AsyncWriteExt, AsyncReadExt}};

use crate::torrent::Sha1Hash;
use crate::utils::AsBytes;


pub mod handshake;
pub use handshake::Handshake;

#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug)]
pub struct Message {
    pub size: usize,
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
        let size = payload.len() + 1;

        Message {
            size,
            id,
            payload,
        }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Message {
        let id = MessageID::from(bytes[0]);
        let payload = bytes[1..].to_vec();

        Message {
            size: payload.len() + 1,
            id,
            payload,
        }
    }

}