use rand::Rng;
use lazy_static::lazy_static;

use tokio::sync::Mutex;
use std::sync::Arc;

use crate::torrent::Torrent;

lazy_static! {
    pub static ref CLIENT: Mutex<Client> = Mutex::new(Client::new());
}

pub struct Client {
    pub id: [u8; 20],
    pub torrents: Vec<Arc<Mutex<Torrent>>>,
} 

impl Client {
    fn create_random_id() -> Vec<u8> {
        let mut id = "TtT-1-0-0-".as_bytes().to_vec();
        for _ in 0..10 {
            let random_symbol = rand::thread_rng().gen_range(0..=255);
            id.push(random_symbol);
        }

        id
    }

    pub fn new() -> Client {
        let id: [u8;20] = Self::create_random_id().try_into().unwrap();

        Client { 

            id: id, 
            torrents: Vec::new() 
        }
    } 

    fn add_torrent(&mut self, torrent: Torrent) {
        self.torrents.push(Arc::new(Mutex::new(torrent)));
    }
}

pub async fn client_get_id() -> [u8; 20] {
    CLIENT.lock().await.id.clone()
}

pub async fn client_add_torrent(torrent: Torrent) {
    CLIENT.lock().await.add_torrent(torrent);
}

pub async fn client_get_torrent(torrent_name: String) -> Option<Arc<Mutex<Torrent>>> {
    let client = CLIENT.lock().await;
    for torrent in client.torrents.iter() {
        if torrent.lock().await.torrent_name == torrent_name {
            return Some(torrent.clone());
        }
    }

    None
}

pub async fn client_get_all_torrents() -> Vec<Arc<Mutex<Torrent>>> {
    CLIENT.lock().await.torrents.clone()
}