use tokio::sync::mpsc;

pub mod torrent_file;
pub mod peer;
pub mod tracker;
pub mod file_saver;

use peer::{Peer, get_peers};
use tracker::Tracker;
use file_saver::FileSaver;
use torrent_file::TorrentFile;

#[derive(Debug)]
pub struct Torrent {
    pub torrent_name: String,
    pub peers: Vec<Peer>,
    pub trackers: Vec<Tracker>,
    pub file_saver: FileSaver,
    pub torrent_file: TorrentFile
}

impl Torrent {
    pub async fn new(torrent_file_name: &str, dest_dir: &str) -> Torrent {
        // TODO: Watch what this buffer does
        let (tx, rx) = mpsc::channel::<Vec<u8>>(100);

        let torrent_name = torrent_file_name.to_string();
        let torrent_file = TorrentFile::new(torrent_file_name);
        let trackers = vec![Tracker::new(&torrent_file).await];
        let file_saver = FileSaver::new(dest_dir, rx);
        let peers = get_peers(&trackers[0], tx).await;

        Torrent { 
            torrent_name,
            torrent_file,
            trackers, 
            file_saver, 
            peers,
        }
    }
}