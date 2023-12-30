use serde::{Serialize, Deserialize};

use std::fmt::Display;

use crate::torrent::{TorrentParser, BencodedValue};

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

    pub async fn from_tracker_response(resp: reqwest::Response) -> Vec<PeerAddress> {
        let bencoded_response = resp.bytes().await.unwrap();
        let bencoded_response = TorrentParser::parse_tracker_response(&bencoded_response);
        
        let bencoded_dict = match bencoded_response {
            BencodedValue::Dict(dict) => dict,
            _ => panic!("Error: Invalid parsed dictionary from tracker response")
        };

        let peer_addresses = match bencoded_dict.get("peers") {
            Some(BencodedValue::ByteAddresses(byte_addresses)) => byte_addresses,
            Some(BencodedValue::Dict(_peer_dict)) => todo!(),
            _ => panic!("Error: Invalid peers key from tracker response")
        };
        
        peer_addresses.to_vec()
    }

}