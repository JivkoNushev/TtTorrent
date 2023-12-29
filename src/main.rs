use anyhow::Result;
use interprocess::local_socket::LocalSocketListener;
use tokio_stream::StreamExt;

use std::io::Read;

use torrent_client::{client::ClientHandle, messager::ClientMessage};

#[tokio::main]
async fn main() -> Result<()> {
    // ------------------------ create socket for client ------------------------

    // remove socket if it exists and create new one
    let _ = tokio::fs::remove_file("/tmp/TtTClient.sock").await;
    let client_socket = match LocalSocketListener::bind("/tmp/TtTClient.sock") {
        Ok(socket) => socket,
        Err(e) => {
            println!("Failed to create socket: {}", e);
            return Ok(());
        }
    };
    let mut stream = tokio_stream::iter(client_socket.incoming());

    // ------------------------ create client ------------------------
    let mut client = ClientHandle::new();
    
    println!("Trying to select");
    loop {
        tokio::select! {
            Some(Ok(mut bytes)) = stream.next() => {
                println!("Received a message");

                let mut message = Vec::new();
                let _ = bytes.read_to_end(&mut message).unwrap();

                let message = match serde_json::from_slice::<ClientMessage>(&message) {
                    Ok(message) => message,
                    Err(e) => {
                        println!("Failed to deserialize message: {}", e);
                        continue;
                    }
                };
                println!("Received message: {:?}", message);

                match message {
                    ClientMessage::DownloadTorrent{src, dst} => {
                        client.client_download_torrent(src, dst).await?;
                    },
                    ClientMessage::Shutdown => {
                        client.client_shutdown().await?;
                        break;
                    },
                    _ => {}
                }
            }
        }
    }

    client.join_handle.await?;
    Ok(())
}