
use torrent_client::client::Client;

#[tokio::main]
async fn main() {
    let (client, downloader_tx, _seeder_tx) = Client::new().await;

    let _ = tokio::join!(
        client.run(),
        tokio::spawn(async move {
            println!("Sending a command to the downloader");
            let _ = downloader_tx.send("torrentfile.torrent".to_string()).await;
        })
    );
}
