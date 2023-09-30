use crate::torrent_file::{BencodedValue, self};

use reqwest;
use std::convert::Infallible;
use sha1::{Sha1, Digest};



fn sha1_hash(info_dict: &BencodedValue) -> [u8; 20] {

    if let BencodedValue::Dict(d) = info_dict {
        let bencoded_dict = info_dict.to_bencoded_format();

        let mut hasher = Sha1::new();
        hasher.update(bencoded_dict);
        hasher.finalize().into()
    }
    else {
        panic!("Trying to hash a non-dictionary");
    }
}   

// TODO: Change to Result type not Option
fn create_tracker_url(torrent_file: &mut BencodedValue) -> Option<String> {
    let mut da = torrent_file.get_from_dict("info");
    let t = da.get_from_dict("pieces");

    let mut pieces_URL_encoded = String::new();
    if let BencodedValue::ByteString(b) = t {
        for i in b {
            pieces_URL_encoded.push_str(&i.url_encoded());
        }
    }

    let ne = torrent_file.get_from_dict("announce");

    let mut url = String::new();
    if let BencodedValue::String(s) = ne {
        url = String::from(s);
        url.push_str("?info_hash=");
        url.push_str(&pieces_URL_encoded);
        url.push_str("&peer_id=1&port=6881&uploaded=0&downloaded=0&left=0&compact=1&event=started");
        println!("URL: {}", url);
        Some(url)
    }
    else {
        None
    }
}

pub fn get_peers(torrent_file: &mut BencodedValue) {
    let url = create_tracker_url(torrent_file).unwrap();

    let res = reqwest::blocking::get(&url).unwrap();
    println!("Status: {}", res.status());
    println!("Headers:\n{:#?}", res.headers());

    let body = res.text().unwrap();
    println!("Body:\n{}", body);

}