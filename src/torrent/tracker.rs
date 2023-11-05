use crate::torrent::torrent_file::{Sha1Hash, TorrentFile};

pub mod tracker_connection;
use tracker_connection::{tracker_url_get, tracker_params_default};

use self::tracker_connection::tracker_hashed_info_dict_get;

#[derive(Debug, Clone)]
pub struct Tracker {
    id: String,
    url: String,
    params: String,
    info_hash: Sha1Hash,
}
impl Tracker {
    pub fn new(torrent_file: &TorrentFile) -> Tracker {
        let info_hash = match tracker_hashed_info_dict_get(torrent_file.get_bencoded_dict()) {
            Ok(info_hash) => info_hash,
            Err(e) => panic!("Error: {e}")
        };
        
        let url = match tracker_url_get(torrent_file.get_bencoded_dict()) {
            Ok(url) => url,
            Err(e) => panic!("Error: {e}")
        };

        let params = tracker_params_default(&info_hash);

        Tracker {
            id: String::from("M-1-0-0CCCCCCCCCCCCC"),
            url,
            params,
            info_hash
        }
    }

    pub fn set_params(&mut self, new_params: String) {
        self.params = new_params;
    }

    pub fn get_url(&self) -> String {
        format!("{}{}", self.url, self.params)
    }

    pub fn get_hashed_info_dict(&self) -> &Sha1Hash {
        &self.info_hash
    }

    pub fn get_id(&self) -> &String {
        &self.id
    }



}
