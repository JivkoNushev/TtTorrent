use interprocess::local_socket::LocalSocketListener;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

use std::io::Read;

use torrent_client::client::Client;
use torrent_client::client::messager::InterProcessMessage;

#[tokio::main] // flavor = "current_thread" ( maybe use one thread if its faster )
async fn main() {
    let (client_tx, client_rx) = mpsc::channel::<InterProcessMessage>(100);

    let client = Client::new(client_tx.clone(), client_rx);
    Client::setup_graceful_shutdown(client_tx.clone());

    tokio::spawn(client.run());
    
    // remove socket if it exists and create new one
    let _ = tokio::fs::remove_file("/tmp/TtTClient.sock").await;
    let client_socket = LocalSocketListener::bind("/tmp/TtTClient.sock").unwrap();
    
    // accept commands
    let mut stream = tokio_stream::iter(client_socket.incoming());
    while let Some(Ok(mut s)) = stream.next().await {
        let mut message = Vec::new();
        let _ = s.read_to_end(&mut message).unwrap();

        let message = match serde_json::from_slice::<InterProcessMessage>(&message) {
            Ok(message) => message,
            Err(e) => {
                println!("Failed to deserialize message: {}", e);
                continue;
            }
        };

        let _ = client_tx.send(message).await;
    }
}