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

    pub fn to_vec(&self) -> Vec<u8> {
        let mut bytes = self.address
            .split('.')
            .map(|octet| octet.parse::<u8>().unwrap())
            .collect::<Vec<u8>>();
        bytes.append(
            self.port
                .parse::<u16>()
                .unwrap()
                .to_be_bytes()
                .to_vec()
                .as_mut()   
        );

        bytes
    }

    pub async fn from_tracker_response(bencoded_response: BencodedValue) -> Result<Vec<PeerAddress>> {
        let bencoded_dict = bencoded_response.try_into_dict()?;

        match bencoded_dict.get(&b"peers".to_vec()) {
            Some(BencodedValue::ByteAddresses(byte_addresses)) => Ok(byte_addresses.to_vec()),
            Some(BencodedValue::Dict(_peer_dict)) => unimplemented!(),
            _ => {
                if let Some(failure) = bencoded_dict.get(&b"failure reason".to_vec()) {
                    tracing::debug!("Failure reason: {}", String::from_utf8(failure.try_into_byte_string()?.to_vec())?);
                    Err(anyhow!("Failure reason: {}", String::from_utf8(failure.try_into_byte_string()?.to_vec())?))
                }
                else {
                    tracing::debug!("Invalid peers key in tracker response");
                    Err(anyhow!("Invalid peers key in tracker response"))
                }
            }
        }
    }

}