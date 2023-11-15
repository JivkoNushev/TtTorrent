use crate::torrent::torrent_file::{ BencodedValue, Sha1Hash };

use crate::client::Client;
use crate::utils::UrlEncodable;


pub(super) fn tracker_url_get(bencoded_dict: &BencodedValue) -> Option<String> {
    if let Some(tracker_announce) = bencoded_dict.get_from_dict("announce") {
        if let BencodedValue::String(tracker_announce) = tracker_announce {
            return Some(tracker_announce.clone());
        }
        None
    } else {
        panic!("No tracker announce key found")
    }
}


pub(super) async fn tracker_params_default(hashed_info_dict: &Sha1Hash) -> String {
    let mut params = String::new();

    params.push_str("?info_hash=");
    params.push_str(&hashed_info_dict.as_url_encoded());

    params.push_str("&peer_id=");
    
    params.push_str(&Client::get_client_id().await.as_url_encoded());

    params.push_str("\
                &port=6881\
                &uploaded=0\
                &downloaded=0\
                &left=0\
                &compact=1\
                &event=started\
    ");
    
    params
}