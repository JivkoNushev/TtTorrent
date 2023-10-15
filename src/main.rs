use tokio::sync::mpsc;

use TtTorrent::torrent_file::{TorrentFile, Sha1Hash};
use TtTorrent::peers::{Peer, get_peers};
use TtTorrent::file_download::FileSaver;
use TtTorrent::tracker::Tracker;

pub async fn download(tracker: Tracker, peers: Vec<Peer>, mut file_saver: FileSaver) {
    match tokio::join!(
        file_saver.start(),
        tokio::spawn(async move {
            for mut peer in peers {
                let cloned_tracker = tracker.clone();
                tokio::spawn(async move {
                    peer.download(cloned_tracker).await;
                });
            }
        }),
    ) {
        ((), Ok(_)) => println!("File downloaded successfully"),
        ((), Err(e)) => println!("Error downloading file: {:?}", e),
    };
}

#[tokio::main]
async fn main() {
    // read the torrent file
    let mut torrent_file = TorrentFile::new("./torrent_files/centOS.torrent");

    let tracker = Tracker::new(&torrent_file);

    // Create a channel fot the file saver
    let (tx, rx) = mpsc::channel::<Vec<u8>>(100);

    // Create a file saver instance 
    let file_saver = FileSaver::new("newfile".to_string(), rx);

    // get the initial peers
    let peers = get_peers(&tracker, tx).await;
    // println!("{:?}", peers);
    
    download(tracker.clone() , peers, file_saver).await;
}
