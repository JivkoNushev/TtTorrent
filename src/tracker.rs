use anyhow::{Result, Context};

use crate::torrent::torrent_file::{ BencodedValue, Sha1Hash };

use crate::client::ClientHandle;
use crate::peer::PeerHandle;
use crate::utils::UrlEncodable;

#[derive(Debug, Clone)]
pub struct Tracker {
    url: String,
}

impl Tracker {
    pub async fn new(url: &str) -> Tracker {
        Tracker {
            url: url.to_string(),
        }
    }

    pub fn get_url(&self, parameters: &str) -> String {
        format!("{}{}", self.url, parameters)
    }

    pub async fn default_params(&mut self, hashed_info_dict: &Sha1Hash, client_id: [u8; 20]) -> Result<reqwest::Response> {
        let tracker_request = self.get_url(&Tracker::tracker_params_default(hashed_info_dict, client_id));
        
        let mut response = reqwest::get("https://1.1.1.1").await.context("error with debug request")?;
        if !crate::DEBUG_MODE {
            response = reqwest::get(&tracker_request).await.context("invalid tracker url")?;
        }

        Ok(response)
    }
}

impl Tracker {
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
    
    pub fn tracker_params_default(hashed_info_dict: &Sha1Hash, client_id: [u8; 20]) -> String {
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
            peer_id = client_id.as_url_encoded()
        }
    }
}