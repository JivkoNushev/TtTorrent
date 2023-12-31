use anyhow::{Result, Context};
use interprocess::local_socket::LocalSocketListener;
use tokio_stream::StreamExt;

use std::io::Read;

use torrent_client::client::ClientHandle;
use torrent_client::messager::ClientMessage;
use torrent_client::utils::valid_src_and_dst;

#[tokio::main]
async fn main() -> Result<()> {
    // ------------------------ create socket for client ------------------------

    // remove socket if it exists and create new one
    let _ = tokio::fs::remove_file(torrent_client::SOCKET_PATH).await.context("couldn't remove old socket")?;
    let client_socket = LocalSocketListener::bind(torrent_client::SOCKET_PATH).context("couldn't bind to local socket")?;
    let mut socket_stream = tokio_stream::iter(client_socket.incoming());

    // ------------------------ create client ------------------------
    let mut client = ClientHandle::new();
    
    loop {
        tokio::select! {
            Some(Ok(mut bytes)) = socket_stream.next() => {
                let mut message = Vec::new();
                if let Err(e) = bytes.read_to_end(&mut message) {
                    eprintln!("Failed to read message from local socket: {}", e);
                    continue;
                }

                let message = match serde_json::from_slice::<ClientMessage>(&message) {
                    Ok(message) => message,
                    Err(e) => {
                        eprintln!("Failed to deserialize message from local socket: {}", e);
                        continue;
                    }
                };

                match message {
                    ClientMessage::Shutdown => {
                        client.client_shutdown().await?;
                        break;
                    },
                    ClientMessage::DownloadTorrent{src, dst} => {
                        // check src and dst
                        if !valid_src_and_dst(&src, &dst) {
                            eprintln!("Invalid src or dst");
                            continue;
                        }

                        client.client_download_torrent(src, dst).await?;
                    },
                    ClientMessage::SendTorrentInfo => {
                        client.client_send_torrent_info().await?;
                    },
                    ClientMessage::TerminalClientClosed => {
                        client.client_terminal_closed().await?;
                    },
                    _ => {}
                }
            }
        }
    }

    client.join().await?;
    Ok(())
}