
use torrent_client::client::Client;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (client, downloader_tx, _seeder_tx) = Client::new().await;

    let _ = tokio::join!(
        client.run(),
        tokio::spawn(async move {
            println!("Sending a torrent file to the downloader");
            let _ = downloader_tx.send("the_fanimatrix.torrent".to_string()).await;
        })
    );
}