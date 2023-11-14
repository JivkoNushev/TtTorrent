pub mod peer_address;

pub use peer_address::PeerAddress;

use crate::torrent::Torrent;

#[derive(Debug)]
pub struct Peer {
    pub id: [u8;20]
}

impl Peer {
    pub async fn new() -> Peer {
        Peer {
            id: [0;20]
        }
    }

    pub async fn get_from_torrent(torrent: &Torrent) -> Vec<Peer> {
        vec![Peer::new().await, Peer::new().await]
    }
}