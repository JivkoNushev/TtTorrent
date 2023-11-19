
use torrent_client::client::Client;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (client, downloader_tx, _seeder_tx) = Client::new();

    let _ = tokio::join!(
        client.run(),
        tokio::spawn(async move {
            println!("Sending a torrent file to the downloader");
            let _ = downloader_tx.send("test_data/foo.torrent".to_string()).await;
        })
    );
}

// 95088b1636648b466f8a209d746eec13b42604f1