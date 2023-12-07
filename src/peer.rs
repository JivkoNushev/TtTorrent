use tokio::sync::Mutex;

use std::fmt::Display;
use std::sync::Arc;

use crate::MAX_PEERS_COUNT;
use crate::torrent::{Torrent, TorrentParser};
use crate::torrent::torrent_file::BencodedValue;
use crate::tracker::Tracker;

pub mod peer_address;
pub use peer_address::PeerAddress;

pub mod peer_messages;


#[derive(Debug, Clone)]
pub struct Peer {
    pub id: [u8;20],
    pub address: String,
    pub port: String,
    pub am_interested: bool,
    pub choking: bool,
}

impl Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.address, self.port)
    }
}

impl Peer {
    pub fn new(peer_address: PeerAddress, id_num: usize) -> Peer {
        Peer {
            id: [id_num as u8;20],
            address: peer_address.address,
            port: peer_address.port,
            am_interested: false,
            choking: true,
        }
    }

    pub async fn get_from_torrent(torrent: &Arc<Mutex<Torrent>>) -> Vec<Peer> {
        if crate::DEBUG_MODE {
            // locally hosted peers
            vec![
                Peer::new(PeerAddress { address: "127.0.0.1".into(), port: "51413".into() }, 0),
                // Peer::new(PeerAddress { address: "192.168.0.28".into(), port: "37051".into() }, 1),
            ]
        }
        else {
            let tracker = Tracker::new(torrent).await;
            Peer::get_peers(&tracker, MAX_PEERS_COUNT).await
        }
    }

    async fn get_peers(tracker: &Tracker, max_peers_count: usize) -> Vec<Peer> {
        let url = tracker.get_url();

        let resp = match reqwest::get(url).await {
            Ok(resp) => resp,
            Err(e) => panic!("Error: tracker couldn't return a response {}", e)
        };
        
        let bencoded_response = resp.bytes().await.unwrap();
        let bencoded_response = TorrentParser::parse_tracker_response(&bencoded_response);
        
        let mut peers = Vec::new();
        
        let bencoded_dict = match bencoded_response {
            BencodedValue::Dict(dict) => dict,
            _ => panic!("Error: Invalid parsed dictionary from tracker response")
        };

        let bencoded_peers = match bencoded_dict.get("peers") {
            Some(BencodedValue::ByteAddresses(byte_addresses)) => byte_addresses,
            Some(BencodedValue::Dict(_peer_dict)) => todo!(),
            _ => panic!("Error: Invalid peers key from tracker response")
        };

        for (i, addr) in bencoded_peers.into_iter().enumerate() {
            if i >= max_peers_count {
                break;
            }

            let peer = Peer::new(addr.clone(), i);
            peers.push(peer);
        }

        peers
    }
}