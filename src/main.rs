use anyhow::{Result, Context};
use interprocess::local_socket::LocalSocketListener;
use tokio_stream::StreamExt;

use std::io::{Read, Write};

use torrent_client::client::ClientHandle;
use torrent_client::messager::{TerminalClientMessage, ClientMessage};
use torrent_client::utils::valid_src_and_dst;

#[tokio::main]
async fn main() -> Result<()> {
    // ------------------------ create socket for client ------------------------

    // remove socket if it exists and create new one
    let _ = tokio::fs::remove_file(torrent_client::SOCKET_PATH).await;
    let client_socket = LocalSocketListener::bind(torrent_client::SOCKET_PATH).context("couldn't bind to local socket")?;
    let mut socket_stream = tokio_stream::iter(client_socket.incoming());

    // ------------------------ create client ------------------------
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ClientMessage>(torrent_client::MAX_CHANNEL_SIZE);
    let mut client = ClientHandle::new(tx);
    
    loop {
        tokio::select! {
            Some(Ok(mut socket)) = socket_stream.next() => {
                let mut message = Vec::new();
                if let Err(e) = socket.read_to_end(&mut message) {
                    eprintln!("Failed to read message from local socket: {}", e);
                    continue;
                }

                if message.is_empty() {
                    eprintln!("Received empty message");
                    continue;
                }
                
                let message = match serde_json::from_slice::<TerminalClientMessage>(&message) {
                    Ok(message) => message,
                    Err(e) => {
                        eprintln!("Failed to deserialize message from local socket: {}", e);
                        continue;
                    }
                };

                match message {
                    TerminalClientMessage::Shutdown => {
                        client.client_shutdown().await?;
                        break;
                    },
                    TerminalClientMessage::Download{src, dst} => {
                        if !valid_src_and_dst(&src, &dst) {
                            eprintln!("Invalid src or dst");
                            continue;
                        }
                        if let Err(e) = client.client_download_torrent(src, dst).await {
                            eprintln!("Failed to send download message to client: {}", e);
                            continue;
                        }
                    },
                    TerminalClientMessage::ListTorrents => {
                        if let Err(e) = client.client_list_torrents().await {
                            eprintln!("Failed to send list torrents message to client: {}", e);
                            continue;
                        }

                        let torrents = match rx.recv().await {
                            Some(ClientMessage::TorrentsInfo{torrents}) => torrents,
                            _ => {
                                eprintln!("Failed to get torrents info");
                                continue;
                            }
                        };

                        let serialized_data = match serde_json::to_string(&TerminalClientMessage::TorrentsInfo{torrents}) {
                            Ok(data) => data,
                            Err(e) => {
                                eprintln!("Failed to serialize torrents info: {}", e);
                                continue;
                            }
                        };
                        if let Err(e) = socket.write_all(serialized_data.as_bytes()) {
                            eprintln!("Failed to write to local socket: {}", e);
                            // TODO: continue
                            return Ok(());
                        }
                    },
                    TerminalClientMessage::TerminalClientClosed => {
                        if let Err(e) = client.client_terminal_closed().await {
                            eprintln!("Failed to send terminal client closed message to client: {}", e);
                            continue;
                        }
                    },
                    _ => {}
                }
            },
            _ = tokio::signal::ctrl_c() => {
                client.client_shutdown().await?;
                break;
            }
        }
    }

    client.join().await?;
    Ok(())
}