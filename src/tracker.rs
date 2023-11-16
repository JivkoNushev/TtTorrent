

mod connection;

use crate::torrent::Torrent;

#[derive(Debug, Clone)]
pub struct Tracker {
    url: String,
    params: String,
}
impl Tracker {
    pub async fn new(torrent: &Torrent) -> Tracker {
        let url = match connection::tracker_url_get(torrent.torrent_file.get_bencoded_dict_ref()) {
            Some(url) => url,
            None => panic!("Error: Invalid tracker url")
        };

        let info_hash = torrent.get_info_hash_ref().await;
        let params = connection::tracker_params_default(info_hash).await;

        Tracker {
            url,
            params,
        }
    }

    pub async fn get_url(&self) -> String {
        format!("{}{}", self.url, self.params)
    }
}