use rand::Rng;

use crate::torrent::Torrent;

pub struct Client {
    pub id: [u8; 20],
    pub torrents: Vec<Torrent>, // Vec<Box<Torrent>> ??
} 

impl Client {
    fn create_random_id() -> [u8] {
        let id = "TtT-1-0-0-".as_bytes().to_vec();
        for _ in 0..10 {
            let random_symbol = rand::thread_rng().gen_range(0..256);
            id.push(random_symbol);
        }

        id
    }

    pub fn new() -> Client {
        let id = create_random_id();

        Client { 
            id: id, 
            torrents: Vec::new() 
        }
    } 

    pub fn add_torrent(&mut self, torrent: &Torrent) {
        self.torrents.push(torrent)
    }
}