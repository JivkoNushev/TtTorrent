use anyhow::{Result, Context, anyhow};
use interprocess::local_socket::tokio::LocalSocketListener;

use torrent_client::client::ClientHandle;
use torrent_client::messager::{TerminalClientMessage, ClientMessage, ExitCode};
use torrent_client::utils::valid_src_and_dst;

mod terminal_utils;
use crate::terminal_utils::TerminalClient;

use tracing::{Level, event, instrument};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ------------------------ create socket for client ------------------------

    // remove socket if it exists and create new one
    let path = std::path::Path::new(torrent_client::SOCKET_PATH);

    let _ = tokio::fs::remove_file(path).await;
    let client_socket = LocalSocketListener::bind(path).context("couldn't bind to local socket")?;

    let mut terminal_client_sockets: Vec<TerminalClient> = Vec::new();
    // ------------------------ create client ------------------------
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ClientMessage>(torrent_client::MAX_CHANNEL_SIZE);
    let mut client = ClientHandle::new(tx);

    // client.client_download_torrent("test_folder/torrents/ospdf.torrent".to_string(), "test_folder/data".to_string()).await?;
    // client.client_download_torrent("test_folder/torrents/torrent_image.torrent".to_string(), "test_folder/data".to_string()).await?;

    loop {
        tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c() => {
                break;
            },
            Ok(socket) = client_socket.accept() => {
                println!("new terminal client connected");
                let mut terminal_client = TerminalClient{socket, client_id: 0};

                let message = match terminal_client.recv_message().await {
                    Ok(message) => message,
                    Err(e) => {
                        terminal_client_sockets.retain(|terminal_client| terminal_client.client_id != terminal_client.client_id);
                        eprintln!("Failed to read from local socket: {}", e);
                        continue;
                    }
                };

                match message {
                    TerminalClientMessage::Shutdown => {
                        client.client_shutdown().await?;
                    },
                    TerminalClientMessage::Download{src, dst} => {
                        if !valid_src_and_dst(&src, &dst) {
                            terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::InvalidSrcOrDst }).await?;
                            return Err(anyhow!("Invalid src or dst"));
                        }
                        if let Err(e) = client.client_download_torrent(src, dst).await {
                            terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::InvalidSrcOrDst }).await?;
                            return Err(anyhow!("Failed to send download message to client: {}", e));
                        }

                        terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::SUCCESS }).await?;
                    },
                    TerminalClientMessage::ListTorrents{client_id} => {
                        println!("client id: {}", client_id);
                        if terminal_client_sockets.is_empty() {
                            client.client_list_torrents().await?;
                        }

                        terminal_client.client_id = client_id;
                        terminal_client_sockets.push(terminal_client);
                    },
                    TerminalClientMessage::TerminalClientClosed{client_id} => {
                        terminal_client_sockets.retain(|terminal_client| terminal_client.client_id != client_id);

                        if terminal_client_sockets.is_empty() {
                            client.client_terminal_closed().await?;
                        }
                    },
                    _ => {}
                }

                println!("terminal client disconnected");
            },
            Some(client_message) = rx.recv() => {
                match client_message {
                    ClientMessage::TorrentsInfo{torrents} => {
                        let message = TerminalClientMessage::TorrentsInfo{torrents};
                        for terminal_client in terminal_client_sockets.iter_mut() {
                            if let Err(e) = terminal_client.send_message(&message).await {
                                terminal_client_sockets.retain(|terminal_client| terminal_client.client_id != terminal_client.client_id);
                                
                                if terminal_client_sockets.is_empty() {
                                    client.client_terminal_closed().await?;
                                }
                                
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
        }
    }

    client.client_shutdown().await?;
    let _ = tokio::fs::remove_file(path).await;

    client.join().await?;

    Ok(())
}