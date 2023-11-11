use TtTorrent::client::{client_get_id, client_add_torrent, client_get_all_torrents};
use TtTorrent::torrent::Torrent;

use tokio::sync::Mutex;
use std::sync::Arc;

pub async fn download(torrent: Arc<Mutex<Torrent>>) {
    println!("Starting to download!");

    let torrent_file_saver = torrent.clone();
    let mut guard =  torrent_file_saver.lock().await;
    let torrent_file_saver = &mut guard.file_saver;
    println!("first here");

    let peers = torrent.clone();

    let peers = &mut peers.lock().await.peers;

    println!("got here");
    match tokio::join!(
        torrent_file_saver.start(),
        tokio::spawn(async move {
            println!("now here");

            for mut peer in peers {
                let cloned_torrent = torrent.clone();
                tokio::spawn(async move {
                    println!("wow here");
                    peer.download(cloned_torrent).await;
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

    for peer in &torrent.peers {
        println!("Peer: {}", peer);
    }

    println!("ID: {:?}", client_get_id().await);

    client_add_torrent(torrent).await;

    for torrent in client_get_all_torrents().await {
        download(torrent).await;
    }
}
