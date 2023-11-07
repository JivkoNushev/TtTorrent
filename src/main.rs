use TtTorrent::client::{client_get_id, client_add_torrent, client_get_all_torrents};
use TtTorrent::torrent::Torrent;

use tokio::sync::Mutex;
use std::sync::Arc;

pub async fn download(torrent: Arc<Mutex<Torrent>>) {
    println!("Starting to download!");

    let torrent_file_saver = torrent.clone();
    let mut torrent_file_saver = torrent_file_saver.lock().await;

    let peers = torrent.clone();
    let peers = peers.lock().await.peers.clone();

    match tokio::join!(
        torrent_file_saver.file_saver.start(),
        tokio::spawn(async move {
            for peer in peers {
                let mut cloned_peer = peer.clone();
                let cloned_torrent = torrent.clone();

                tokio::spawn(async move {
                    cloned_peer.download(cloned_torrent).await;
                });
            }
        }),
    ) {
        ((), Ok(_)) => println!("File downloaded successfully"),
        ((), Err(e)) => println!("Error downloading file: {:?}", e),
    };
}


#[tokio::main(flavor = "current_thread")]
async fn main() {
    let torrent = Torrent::new("torrent_files/the_fanimatrix.torrent", "newfile").await;
    println!("{:?}", torrent.peers);

    println!("ID: {:?}", client_get_id().await);

    client_add_torrent(torrent);

    for torrent in client_get_all_torrents().await {
        download(torrent).await;
    }
}
