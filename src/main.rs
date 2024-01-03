use anyhow::{Result, Context};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
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
    
    use std::io::{BufReader, BufRead};

    struct TerminalClient {
        socket: LocalSocketStream,
        client_id: u32,
    }

    let mut sending_to_terminal_client = false;
    let mut terminal_client_sockets: Vec<TerminalClient> = Vec::new();

    loop {
        tokio::select! {
            Some(Ok(mut socket)) = socket_stream.next() => {
                println!("Received connection from local socket");
                let mut reader = BufReader::new(&mut socket);

                let mut message = String::new();
                if let Err(e) = reader.read_line(&mut message) {
                    eprintln!("Failed to read message from local socket: {}", e);
                    continue;
                }
                if message.is_empty() {
                    eprintln!("Received empty message");
                    continue;
                }

                println!("Received message: {}", message);
                
                let message = match serde_json::from_slice::<TerminalClientMessage>(&message.as_bytes()) {
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
                    TerminalClientMessage::ListTorrents{client_id} => {
                        println!("sending list torrents message to client");
                        sending_to_terminal_client = true;
                        terminal_client_sockets.push(TerminalClient{socket, client_id});
                    },
                    TerminalClientMessage::TerminalClientClosed{client_id} => {
                        terminal_client_sockets.retain(|terminal_client| terminal_client.client_id != client_id);

                        if terminal_client_sockets.is_empty() {
                            if let Err(e) = client.client_terminal_closed().await {
                                eprintln!("Failed to send terminal client closed message to client: {}", e);
                                continue;
                            }
                        }
                    },
                    _ => {}
                }
            },
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                println!("checking for messages from client: {sending_to_terminal_client:?}");
                if sending_to_terminal_client {
                    println!("sending to terminal client");

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

                    println!("sending: {}", serialized_data);

                    for terminal_client in terminal_client_sockets.iter_mut() {
                        if let Err(e) = terminal_client.socket.write_all(serialized_data.as_bytes()) {
                            eprintln!("Failed to write to local socket: {}", e);
                            // TODO: continue
                            return Ok(());
                        }
                    }
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