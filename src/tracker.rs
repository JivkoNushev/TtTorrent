use std::num::ParseIntError;

use crate::torrent_file::{BencodedValue, parse_to_torrent_file, Sha1Hash, parse_tracker_response};

use reqwest;
use sha1::{Sha1, Digest};

fn sha1_hash(value: Vec<u8>) -> Sha1Hash {
    let mut hasher = Sha1::new();
    hasher.update(value);
    Sha1Hash::new(&hasher.finalize())
}   

fn create_peer_id(id: String) -> String {
    let padding_len = 20 - id.len();
    let mut peer_id = id.clone();
    let padding: String = vec!['C';padding_len].into_iter().collect();
    peer_id.push_str(&padding);
    println!("peer id {peer_id}");
    peer_id
}

// TODO: Change to Result type insted of Option
fn create_tracker_url(torrent_file: &mut BencodedValue) -> Option<String> {
    if let BencodedValue::String(tracker_announce) = torrent_file.get_from_dict("announce") {
        if let BencodedValue::Dict(info_dict) = torrent_file.get_from_dict("info") {
            let bencoded_info_dict = parse_to_torrent_file(&torrent_file.get_from_dict("info"));
            let hashed_dict_url_encoded = sha1_hash(bencoded_info_dict).as_url_encoded();

            let mut tracker_url = tracker_announce;

            tracker_url.push_str("?info_hash=");
            tracker_url.push_str(&hashed_dict_url_encoded);

            tracker_url.push_str("&peer_id=");
            tracker_url.push_str(create_peer_id("GEY".to_string()).as_str());

            tracker_url.push_str("&port=6881&uploaded=0&downloaded=0&left=0&compact=1&event=started");
            Some(tracker_url)
        }
        else {
            None
        }
        
    }
    else {
        None
    }
}

#[derive(Default)]
struct Peer {
    id: String,
    address: String,
    port: String,
    am_choking: bool,
    am_interested: bool,
    peer_choking: bool,
    peer_interested: bool,
}

impl Peer {
    pub fn new() -> Peer {
        Peer::default()
    }

    // needs to be async
    pub fn download(&self) {

    }
}

pub fn get_peers(torrent_file: &mut BencodedValue) {
    let url = create_tracker_url(torrent_file).unwrap();

    // println!("URL: {}", url);

    let mut res = reqwest::blocking::get(url).unwrap();

    let a = res.bytes().unwrap();

    // println!("Response:\n{:?}", String::from_utf8_lossy(&a));

    let result_dict = parse_tracker_response(&a);

    println!("{:?}", result_dict);

}
