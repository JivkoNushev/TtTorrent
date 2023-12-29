use torrent_client::client::ClientHandle;

#[tokio::main]
async fn main() {
    let mut client = ClientHandle::new();
    client.download_torrent("./test_folder/torrents/import.torrent".to_string(), "./test_folder/data".to_string()).await.unwrap();

    client.join_handle.await.unwrap();
}