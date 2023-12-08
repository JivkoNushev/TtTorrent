use tokio::sync::mpsc;

use torrent_client::client::Client;
use torrent_client::client::messager::{ InterProcessMessage, MessageType };

#[tokio::main] // flavor = "current_thread" ( maybe use one thread if its faster )
async fn main() {
    let (client_tx, client_rx) = mpsc::channel::<InterProcessMessage>(100);

    let client = Client::new(client_tx.clone(), client_rx);

    let _ = tokio::join!(
        client.run(),
        tokio::spawn(async move {
            let download_torrent_message = InterProcessMessage::new(
                MessageType::DownloadTorrent,
                // "test_torrents/folder_torrent.torrent".to_string(),
                "test_torrents/torrent_image.torrent".to_string(),
                "test_data/folder_with_files/".to_string().into_bytes()
            );

            let _ = client_tx.send(download_torrent_message).await;
        })
    );
}