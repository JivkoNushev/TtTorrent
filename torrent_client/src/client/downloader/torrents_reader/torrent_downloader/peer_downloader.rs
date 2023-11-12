use tokio::sync::mpsc;

#[derive(Debug)]
pub struct Peer {}

impl Peer {
    // pub async fn new() -> Peer {}

    pub async fn get_from_torrent(torrent: &Torrent) -> Vec<Peer> {
        vec![Peer{}, Peer{}]
    }
}

#[derive(Debug, Clone)]
pub struct Torrent {
    torrent_name: String
}

impl Torrent {
    // pub async fn new() -> Torrent {}

    pub async fn parse_from_file(torrent_name: String) -> Torrent {
        Torrent {
            torrent_name
        }
    }
}

pub struct PeerDownloader {
    peer: Peer,
    torrent: Torrent,
    writer_tx: mpsc::Sender<String>
}

impl PeerDownloader {
    pub async fn new(torrent: Torrent, peer: Peer, writer_tx: mpsc::Sender<String>) -> PeerDownloader {
        PeerDownloader {
            peer,
            torrent,
            writer_tx
        }
    } 

    pub async fn run(self) {
        self.writer_tx.send("Hello from PeerDownloader".to_string()).await.unwrap();
    }
}
