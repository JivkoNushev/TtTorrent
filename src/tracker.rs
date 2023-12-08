use tokio::sync::Mutex;

use std::sync::Arc;

use crate::torrent::Torrent;

mod connection;
use connection::{tracker_params_default, tracker_url_get};

#[derive(Debug, Clone)]
pub struct Tracker {
    url: String,
    params: String,
}

impl Tracker {
    pub async fn new(torrent: &Arc<Mutex<Torrent>>) -> Tracker {
        let torrent_file = torrent.lock().await.torrent_file.clone();
        let info_hash = torrent.lock().await.get_info_hash_ref().clone();

        let url = match tracker_url_get(torrent_file.get_bencoded_dict_ref()) {
            Some(url) => url,
            None => panic!("Error: Invalid tracker url")
        };

        let params = tracker_params_default(&info_hash).await;

        Tracker {
            url,
            params,
        }
    }

    pub fn get_url(&self) -> String {
        format!("{}{}", self.url, self.params)
    }
}