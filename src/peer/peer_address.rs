use serde::{Serialize, Deserialize};
use anyhow::{anyhow, Result};

use std::fmt::Display;

use crate::utils::bencode::BencodedValue;

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

    pub async fn from_tracker_response(bencoded_response: BencodedValue) -> Result<Vec<PeerAddress>> {
        let bencoded_dict = bencoded_response.try_into_dict()?;

        match bencoded_dict.get(&b"peers".to_vec()) {
            Some(BencodedValue::ByteAddresses(byte_addresses)) => Ok(byte_addresses.to_vec()),
            Some(BencodedValue::Dict(_peer_dict)) => todo!(),
            _ => {
                if let Some(failure) = bencoded_dict.get(&b"failure reason".to_vec()) {
                    // TODO: better capture error no peers found
                    Err(anyhow!("Failure reason: {}", String::from_utf8(failure.try_into_byte_string()?.to_vec())?))
                }
                else {
                    Err(anyhow!("Invalid peers key in tracker response"))
                }
            }
        }
    }

}