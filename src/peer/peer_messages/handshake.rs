
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
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Handshake {
        Handshake {
            protocol_len: 19,
            protocol: *b"BitTorrent protocol",
            reserved: [0; 8],
            info_hash,
            peer_id,
        }
    }

    pub fn from_bytes(handshake_bytes: Vec<u8>) -> Handshake {
        if handshake_bytes.len() != 68 {
            panic!("Error: invalid handshake length");
        }

        Handshake {
            protocol_len: handshake_bytes[0],
            protocol: handshake_bytes[1..20].try_into().unwrap(),
            reserved: handshake_bytes[20..28].try_into().unwrap(),
            info_hash: handshake_bytes[28..48].try_into().unwrap(),
            peer_id: handshake_bytes[48..68].try_into().unwrap(),
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