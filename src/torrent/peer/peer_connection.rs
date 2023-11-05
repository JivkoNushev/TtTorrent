use tokio::sync::mpsc;

use crate::torrent::{
    torrent_file::{BencodedValue, parse_tracker_response}, 
    tracker::Tracker, 
};

use super::Peer;

pub async fn get_peers(tracker: &Tracker, file_queue_tx: mpsc::Sender<Vec<u8>>) -> Vec<Peer> {
    let url = tracker.get_url();

    let resp = reqwest::get(url).await.unwrap();
    
    let bencoded_response = resp.bytes().await.unwrap();
    
    // println!("Tracker response:");
    // print_as_string(&bencoded_response.to_vec());

    let bencoded_dict = parse_tracker_response(&bencoded_response);
    
    let mut peer_array: Vec<Peer> = Vec::new();
    if let BencodedValue::Dict(dict) = bencoded_dict {
        if let BencodedValue::ByteAddresses(byte_addresses) = dict.get("peers").unwrap() {
            // TODO: try removing the clone for the address
            // TODO: get a specific number of addresses
            for (i, addr) in byte_addresses.into_iter().enumerate() {
                if i > 1 {
                    break;
                }
                let peer = Peer::new(addr.clone(), i, file_queue_tx.clone());
                peer_array.push(peer);
            }
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

pub fn peer_create_id(id: usize) -> [u8;20] {
    let mut peer_id = [0u8;20];
    let mut id_bytes = id.to_string().into_bytes();
    id_bytes.resize(20, 0);
    peer_id.copy_from_slice(&id_bytes[..]);
    peer_id
}