use serde::{Serialize, Deserialize};

use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerAddress {
    pub address: String,
    pub port: String
}

impl Display for PeerAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.address, self.port)
    }
}

impl PeerAddress {
    pub fn new(peer_address: [u8; 6]) -> PeerAddress {
        let address = peer_address[..4]
            .iter()
            .map(|&byte| byte.to_string())
            .collect::<Vec<String>>()
            .join(".");

        let port = u16::from_be_bytes(peer_address[4..].try_into().unwrap()).to_string();

        PeerAddress { address, port }
    }
}