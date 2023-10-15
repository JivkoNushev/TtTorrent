use tokio::sync::mpsc;

use crate::{
    torrent_file::{BencodedValue, parse_tracker_response}, 
    tracker::Tracker
};

use super::Peer;

pub async fn get_peers(tracker: &Tracker, file_queue_tx: mpsc::Sender<Vec<u8>>) -> Vec<Peer> {
    let url = tracker.get_url();

    let resp = reqwest::get(url).await.unwrap();

    let bencoded_response = resp.bytes().await.unwrap();

    let bencoded_dict = parse_tracker_response(&bencoded_response);
    
    let mut peer_array: Vec<Peer> = Vec::new();
    if let BencodedValue::Dict(dict) = bencoded_dict {
        if let BencodedValue::ByteAddresses(byte_addresses) = dict.get("peers").unwrap() {
            // TODO: try removing the clone for the address
            // TODO: get a specific number of addresses
            for (i, addr) in byte_addresses.into_iter().enumerate() {
                let peer = Peer::new(addr.clone(), peer_create_id(i.to_string()), file_queue_tx.clone());
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

pub fn peer_create_id(id: String) -> String {
    let padding_len = 20 - id.len();
    let mut peer_id = id.clone();
    let padding: String = vec!['C';padding_len].into_iter().collect();
    peer_id.push_str(&padding);

    peer_id
}