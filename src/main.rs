use torrent_client::client::Client;

#[tokio::main] // flavor = "current_thread" ( maybe use one thread if its faster )
async fn main() {
    let (client, downloader_tx, _seeder_tx) = Client::new();

    let _ = tokio::join!(
        client.run(),
        tokio::spawn(async move {
            println!("Sending a torrent file to the downloader");
            let _ = downloader_tx.send(("test_torrents/torrent_image.torrent".to_string(), "test_data/folder_with_files/".to_string())).await;
        })
    );
}