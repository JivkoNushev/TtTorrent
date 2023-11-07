use anyhow::{Result, bail};

use crate::torrent::torrent_file::{BencodedValue, parse_to_torrent_file, Sha1Hash};


pub(super) fn tracker_url_get(bencoded_dict: &BencodedValue) -> Result<String> {
    if let BencodedValue::String(tracker_announce) = bencoded_dict.get_from_dict("announce") {
        Ok(tracker_announce)
    } else {
        bail!("Invalid announce found when getting the tracker url")
    }
}

pub(super) fn tracker_params_default(hashed_info_dict: &Sha1Hash) -> String {
    let mut params = String::new();

    params.push_str("?info_hash=");
    params.push_str(&hashed_info_dict.as_url_encoded());

    params.push_str("&peer_id=");
    
    params.push_str("M-1-0-0CCCCCCCCCCCCC");

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

