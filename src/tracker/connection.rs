use crate::torrent::torrent_file::{ BencodedValue, Sha1Hash };

use crate::client::Client;
use crate::utils::UrlEncodable;

pub fn tracker_url_get(bencoded_dict: &BencodedValue) -> Option<String> {
    let tracker_announce = match bencoded_dict.get_from_dict("announce") {
        Some(tracker_announce) => tracker_announce,
        None => panic!("No tracker announce key found")
    };

    let tracker_announce = match tracker_announce {
        BencodedValue::ByteString(tracker_announce) => tracker_announce,
        _ => panic!("No tracker announce key found")
    };

    let tracker_announce = match String::from_utf8(tracker_announce.clone()) {
        Ok(tracker_announce) => tracker_announce,
        Err(_) => panic!("Tracker announce key is not a valid utf8 string")
    };

    Some(tracker_announce)
}


pub async fn tracker_params_default(hashed_info_dict: &Sha1Hash) -> String {
    format!{
        "?info_hash={info_hash}\
        &peer_id={peer_id}\
        &port=6881\
        &uploaded=0\
        &downloaded=0\
        &left=0\
        &compact=1\
        &event=started",
        info_hash = hashed_info_dict.as_url_encoded(),
        peer_id = Client::get_client_id().await.as_url_encoded()
    }
}