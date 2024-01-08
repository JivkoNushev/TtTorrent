use std::sync::Arc;

use anyhow::{Result, Context, anyhow};
use interprocess::local_socket::tokio::LocalSocketListener;

use tokio::sync::Mutex;
use torrent_client::client::ClientHandle;
use torrent_client::messager::{TerminalClientMessage, ClientMessage, ExitCode};
use torrent_client::utils::valid_src_and_dst;

mod terminal_utils;
use crate::terminal_utils::TerminalClient;

#[tokio::main]
async fn main() -> Result<()> {
    // ------------------------ create socket for client ------------------------

    // remove socket if it exists and create new one
    let _ = tokio::fs::remove_file(torrent_client::SOCKET_PATH).await;
    let client_socket = LocalSocketListener::bind(torrent_client::SOCKET_PATH).context("couldn't bind to local socket")?;

    let terminal_client_sockets: Arc<Mutex<Vec<TerminalClient>>> = Arc::new(Mutex::new(Vec::new()));
    // ------------------------ create client ------------------------
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ClientMessage>(torrent_client::MAX_CHANNEL_SIZE);
    let client = Arc::new(Mutex::new(ClientHandle::new(tx)));

    let mut terminal_client_handles = Vec::new();

    loop {
        tokio::select! {
            Ok(socket) = client_socket.accept() => {
                let terminal_client_sockets = Arc::clone(&terminal_client_sockets);
                let client = Arc::clone(&client);

                let handle = tokio::spawn(async move {
                    println!("new terminal client connected");
                    let mut terminal_client = TerminalClient{socket, client_id: 0};
    
                    let message = match terminal_client.recv_message().await {
                        Ok(message) => message,
                        Err(e) => {
                            terminal_client_sockets.lock().await.retain(|terminal_client| terminal_client.client_id != terminal_client.client_id);
                            return Err(anyhow!("Failed to read message from local socket: {}", e))
                        }
                    };
    
                    match message {
                        TerminalClientMessage::Shutdown => {
                            client.lock().await.client_shutdown().await?;
                        },
                        TerminalClientMessage::Download{src, dst} => {
                            if !valid_src_and_dst(&src, &dst) {
                                terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::InvalidSrcOrDst }).await?;
                                return Err(anyhow!("Invalid src or dst"));
                            }
                            if let Err(e) = client.lock().await.client_download_torrent(src, dst).await {
                                terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::InvalidSrcOrDst }).await?;
                                return Err(anyhow!("Failed to send download message to client: {}", e));
                            }
    
                            terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::SUCCESS }).await?;
                        },
                        TerminalClientMessage::ListTorrents{client_id} => {
                            println!("client id: {}", client_id);
                            if terminal_client_sockets.lock().await.is_empty() {
                                println!("sending list torrents message to client");
                                client.lock().await.client_list_torrents().await?;
                            }
    
                            terminal_client.client_id = client_id;
                            terminal_client_sockets.lock().await.push(terminal_client);
                        },
                        TerminalClientMessage::TerminalClientClosed{client_id} => {
                            terminal_client_sockets.lock().await.retain(|terminal_client| terminal_client.client_id != client_id);
    
                            if terminal_client_sockets.lock().await.is_empty() {
                                client.lock().await.client_terminal_closed().await?;
                            }
                        },
                        _ => {}
                    }

                    println!("terminal client disconnected");
                    Ok(())
                });

                terminal_client_handles.push(handle);
            },
            Some(client_message) = rx.recv() => {
                match client_message {
                    ClientMessage::TorrentsInfo{torrents} => {
                        let message = TerminalClientMessage::TorrentsInfo{torrents};
                        println!("sending torrents info to terminal client");

                        for terminal_client in terminal_client_sockets.lock().await.iter_mut() {
                            if let Err(e) = terminal_client.send_message(&message).await {
                                terminal_client_sockets.lock().await.retain(|terminal_client| terminal_client.client_id != terminal_client.client_id);
                                eprintln!("Failed to write to local socket: {}", e);
                                break;
                            }
                        }
                    },
                    ClientMessage::Shutdown => {
                        break;
                    },
                    _ => {
                        eprintln!("Received invalid message from client");
                    }
                }
            },
            _ = tokio::signal::ctrl_c() => {
                client.lock().await.client_shutdown().await?;
                break;
            }
        }
    }

    let _ = tokio::fs::remove_file(torrent_client::SOCKET_PATH).await;

    for handle in terminal_client_handles {
        if let Err(e) = handle.await {
            eprintln!("Failed to join terminal client handle: {:?}", e);
        }
    }

    client.lock_owned().await.client_shutdown().await?;

    Ok(())
}