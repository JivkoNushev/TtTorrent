pub mod peer_address;
pub mod peer_messages;

pub use peer_address::PeerAddress;
pub use peer_messages::Handshake;

use crate::torrent::{ Torrent, TorrentParser };
use crate::torrent::torrent_file::BencodedValue;
use crate::tracker::Tracker;

#[derive(Debug)]
pub struct Peer {
    pub id: [u8;20],
    pub address: String,
    pub port: String,
    am_interested: bool,
    peer_choking: bool,
}

impl Peer {
    pub async fn new(peer_address: PeerAddress, id_num: usize) -> Peer {
        
        let id: [u8; 20] = [id_num as u8;20];

        Peer {
            id,
            address: peer_address.address,
            port: peer_address.port,
            am_interested: false,
            peer_choking: true,
        }
    }

    pub async fn get_from_torrent(torrent: &Torrent) -> Vec<Peer> {
        let tracker = Tracker::new(&torrent).await;
        
        Peer::get_peers(&tracker).await
    }


    async fn get_peers(tracker: &Tracker) -> Vec<Peer> {
        let url = tracker.get_url().await;

        let resp = reqwest::get(url).await.unwrap();
        
        let bencoded_response = resp.bytes().await.unwrap();
        
        let bencoded_response = TorrentParser::parse_tracker_response(&bencoded_response).await;
        
        let mut peer_array: Vec<Peer> = Vec::new();
        if let BencodedValue::Dict(dict) = bencoded_response {

            let value = dict.get("peers");
            
            if let Some(BencodedValue::ByteAddresses(byte_addresses)) = value {
                for (i, addr) in byte_addresses.into_iter().enumerate() {
                    // TODO: get a specific number of addresses
                    if i >= 20 {
                        break;
                    }

                    // TODO: try removing the clone for the address
                    let peer = Peer::new(addr.clone(), i).await;
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