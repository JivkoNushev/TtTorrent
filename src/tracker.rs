use crate::torrent_file::{BencodedValue, parse_to_torrent_file, Sha1Hash};

use reqwest;
use sha1::{Sha1, Digest};

fn sha1_hash(value: Vec<u8>) -> Sha1Hash {
    let mut hasher = Sha1::new();
    hasher.update(value);
    Sha1Hash::new(&hasher.finalize())
}   

// TODO: Change to Result type insted of Option
fn create_tracker_url(torrent_file: &mut BencodedValue) -> Option<String> {
    let info_value = torrent_file.get_from_dict("info");

    let bencoded_info_dict = parse_to_torrent_file(&info_value);

    let hashed_dict = sha1_hash(bencoded_info_dict);

    let hashed_dict_url_encoded = hashed_dict.as_url_encoded();

    let tracker_announce: BencodedValue = torrent_file.get_from_dict("announce");

    if let BencodedValue::String(s) = tracker_announce {
        let mut tracker_url = String::from(s);
        tracker_url.push_str("?info_hash=");
        tracker_url.push_str(&hashed_dict_url_encoded);
        tracker_url.push_str("&peer_id=1&port=6881&uploaded=0&downloaded=0&left=0&compact=1&event=started");
        Some(tracker_url)
    }
    else {
        None
    }
}

use std::io::Read;

pub fn get_peers(torrent_file: &mut BencodedValue) {
    let url = create_tracker_url(torrent_file).unwrap();

    println!("URL: {}", url);

    let mut res = reqwest::blocking::get(url).unwrap();
    let mut body = String::new();
    res.read_to_string(&mut body).unwrap();

    println!("Body:\n{:?}", res);

}