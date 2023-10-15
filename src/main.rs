use TtTorrent::peers::peer_connection::get_peers;
use TtTorrent::tracker::Tracker;
use tokio::sync::mpsc;

use TtTorrent::torrent_file::{BencodedValue, TorrentFile, Sha1Hash};
use TtTorrent::peers::Peer;
use TtTorrent::file_download::FileSaver;

pub async fn download(hashed_info_dict: Sha1Hash, peers: Vec<Peer>, mut file_saver: FileSaver) {
    match tokio::join!(
        file_saver.start(),
        tokio::spawn(async move {
            for peer in peers {
                let hashed_info_dict = hashed_info_dict.clone();
                tokio::spawn(async move {
                    peer.download(hashed_info_dict).await;
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
    
    download(tracker.get_hashed_info_dict().clone(), peers, file_saver).await;
}
