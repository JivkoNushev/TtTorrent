pub mod peer_address;
pub mod peer_messages;

use std::fmt::Display;
use std::sync::Arc;

pub use peer_address::PeerAddress;
use tokio::sync::Mutex;

use crate::torrent::{ Torrent, TorrentParser };
use crate::torrent::torrent_file::BencodedValue;
use crate::tracker::Tracker;

#[derive(Debug)]
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
        
        let id: [u8; 20] = [id_num as u8;20];

        Peer {
            id,
            address: peer_address.address,
            port: peer_address.port,
            am_interested: false,
            choking: true,
        }
    }

    pub async fn get_from_torrent(torrent: &Arc<Mutex<Torrent>>) -> Vec<Peer> {
        if crate::DEBUG_MODE {
            vec![
                Peer::new(PeerAddress { address: "127.0.0.1".into(), port: "51413".into() }, 0),
                // Peer::new(PeerAddress { address: "192.168.0.28".into(), port: "37051".into() }, 1),
            ]
        }
        else {
            let tracker = Tracker::new(torrent).await;
            Peer::get_peers(&tracker).await
        }
    }

    async fn get_peers(tracker: &Tracker) -> Vec<Peer> {
        let url = tracker.get_url();

        let resp = match reqwest::get(url).await {
            Ok(resp) => resp,
            Err(e) => panic!("Error: tracker couldn't return a response {}", e)
        };
        
        let bencoded_response = resp.bytes().await.unwrap();
        
        let bencoded_response = TorrentParser::parse_tracker_response(&bencoded_response);
        
        let mut peer_array: Vec<Peer> = Vec::new();
        if let BencodedValue::Dict(dict) = bencoded_response {

            let value = dict.get("peers");
            
            if let Some(BencodedValue::ByteAddresses(byte_addresses)) = value {
                for (i, addr) in byte_addresses.into_iter().enumerate() {
                    // TODO: get a specific number of addresses
                    if i >= 200 {
                        break;
                    }

                    // TODO: try removing the clone for the address
                    let peer = Peer::new(addr.clone(), i);
                    peer_array.push(peer);
                }
            }
            else if let Some(BencodedValue::Dict(_peer_dict)) = value {
                todo!();
            }
            else {
                panic!("Error: Invalid peers key from tracker response");
            }
        }
        else {
            panic!("Error: Invalid parsed dictionary from tracker response");
        }

        peer_array
    }
}