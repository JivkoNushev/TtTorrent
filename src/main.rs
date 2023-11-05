use tokio::sync::mpsc;

use TtTorrent::torrent::Torrent;

pub async fn download(mut torrent: Torrent) {
    println!("Starting to download!");
    match tokio::join!(
        torrent.file_saver.start(),
        tokio::spawn(async move {
            for mut peer in torrent.peers {
                let cloned_tracker = torrent.trackers[0].clone();
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

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // read the torrent file
    let torrent = Torrent::new("torrent_files/the_fanimatrix.torrent", "newfile").await;
    println!("{:?}", torrent.peers);
    download(torrent).await;
}
