use anyhow::{Result, Context};
use interprocess::local_socket::LocalSocketListener;

use torrent_client::client::ClientHandle;
use torrent_client::messager::{TerminalClientMessage, ClientMessage, ExitCode};
use torrent_client::utils::{TerminalClient, valid_src_and_dst};

#[tokio::main]
async fn main() -> Result<()> {
    // ------------------------ create socket for client ------------------------

    // remove socket if it exists and create new one
    let _ = tokio::fs::remove_file(torrent_client::SOCKET_PATH).await;
    let client_socket = LocalSocketListener::bind(torrent_client::SOCKET_PATH).context("couldn't bind to local socket")?;
    client_socket.set_nonblocking(true).context("couldn't set nonblocking mode")?;

    let mut terminal_client_sockets: Vec<TerminalClient> = Vec::new();
    let mut sending_interval = tokio::time::interval(std::time::Duration::from_secs(torrent_client::INTERVAL_SECS));
    // ------------------------ create client ------------------------
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ClientMessage>(torrent_client::MAX_CHANNEL_SIZE);
    let mut client = ClientHandle::new(tx);

    loop {
        tokio::select! {
            Ok(socket) = async { client_socket.accept() } => {
               // println!("new terminal client connected");
                let mut terminal_client = TerminalClient{socket, client_id: 0};

                let message = match terminal_client.read_message() {
                    Ok(message) => message,
                    Err(e) => {
                        terminal_client_sockets.retain(|terminal_client| terminal_client.client_id != terminal_client.client_id);

                        eprintln!("Failed to read message from local socket: {}", e);
                        continue;
                    }
                };

               // println!("Received message from terminal client: {:?}", message);

                match message {
                    TerminalClientMessage::Shutdown => {
                       // println!("sending shutdown message to client");
                        client.client_shutdown().await?;
                        break;
                    },
                    TerminalClientMessage::Download{src, dst} => {
                        if !valid_src_and_dst(&src, &dst) {
                            eprintln!("Invalid src or dst");

                            if let Err(e) = terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::InvalidSrcOrDst }) {
                                eprintln!("Failed to send status message to client: {}", e);
                            }

                            continue;
                        }
                        if let Err(e) = client.client_download_torrent(src, dst).await {
                            eprintln!("Failed to send download message to client: {}", e);

                            if let Err(e) = terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::InvalidSrcOrDst }) {
                                eprintln!("Failed to send status message to client: {}", e);
                            }
                            
                            continue;
                        }

                        if let Err(e) = terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::SUCCESS }) {
                            eprintln!("Failed to send status message to client: {}", e);
                        }
                    },
                    TerminalClientMessage::ListTorrents{client_id} => {
                       // println!("sending list torrents message to client");
                        terminal_client.client_id = client_id;

                        terminal_client_sockets.push(terminal_client);
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
            _ = sending_interval.tick() => {
                if !terminal_client_sockets.is_empty() {
                   // println!("sending to terminal client");

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

                    let message = TerminalClientMessage::TorrentsInfo{torrents};
                    for terminal_client in terminal_client_sockets.iter_mut() {
                        if let Err(e) = terminal_client.send_buffered_message(&message) {
                            terminal_client_sockets.retain(|terminal_client| terminal_client.client_id != terminal_client.client_id);
                            eprintln!("Failed to write to local socket: {}", e);
                            break;
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

    let _ = tokio::fs::remove_file(torrent_client::SOCKET_PATH).await;

    client.join().await?;
    Ok(())
}