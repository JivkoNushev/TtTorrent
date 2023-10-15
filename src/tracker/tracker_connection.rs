use anyhow::{Result, bail};

use crate::{
    torrent_file::{BencodedValue, parse_to_torrent_file, Sha1Hash}, 
    utils::sha1_hash, peers::peer_create_id
};

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
    params.push_str(&peer_create_id("M-1-0-0".to_string()).as_str());

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

pub(super) fn tracker_hashed_info_dict_get(bencoded_dict: &BencodedValue) -> Result<Sha1Hash> {
    if let BencodedValue::Dict(_) = bencoded_dict.get_from_dict("info") {
        let bencoded_info_dict = parse_to_torrent_file(&bencoded_dict.get_from_dict("info"));
        Ok(sha1_hash(bencoded_info_dict))
    }
    else {
        bail!("Invalid dictionary in info key when getting the tracker params")
    }
}