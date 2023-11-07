use crate::torrent::torrent_file::{Sha1Hash, TorrentFile};

pub mod tracker_connection;
use tracker_connection::{tracker_url_get, tracker_params_default};

#[derive(Debug, Clone)]
pub struct Tracker {
    url: String,
    params: String,
}
impl Tracker {
    pub async fn new(torrent_file: &TorrentFile) -> Tracker {
        let url = match tracker_url_get(torrent_file.get_bencoded_dict()) {
            Ok(url) => url,
            Err(e) => panic!("Error: {e}")
        };

        let info_hash = torrent_file.get_info_hash().await;
        let params = tracker_params_default(&info_hash);

        Tracker {
            url,
            params,
        }
    }

    pub async fn get_url(&self) -> String {
        format!("{}{}", self.url, self.params)
    }
}
