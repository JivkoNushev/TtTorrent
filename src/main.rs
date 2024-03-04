use anyhow::{Result, Context, anyhow};
use interprocess::local_socket::tokio::LocalSocketListener;

use torrent_client::client::ClientHandle;
use torrent_client::client_options;
use torrent_client::messager::{TerminalClientMessage, ClientMessage};
use torrent_client::utils::{ExitCode, valid_src_and_dst};
use torrent_client::utils::terminal::TerminalClient;

use std::path::Path;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ------------------------ setup options for client ------------------------

    client_options::setup_options(std::env::args());

    // ------------------------ setup tracing ------------------------
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(unsafe { torrent_client::CLIENT_OPTIONS.tracing_level })
        .pretty()
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    // ------------------------ create socket for client ------------------------
    let sock_path = unsafe { torrent_client::CLIENT_OPTIONS.socket_path.clone() };
    let sock_path = Path::new(&sock_path);

    // remove socket if it exists and create new one
    let _ = tokio::fs::remove_file(sock_path).await;
    std::fs::create_dir_all(sock_path.parent().context("couldn't get parent of socket path")?).context("couldn't create parent directory for socket")?;
    let client_socket = LocalSocketListener::bind(sock_path).context("couldn't bind to local socket")?;

    let mut terminal_client_sockets: Vec<TerminalClient> = Vec::new();

    // ------------------------ create client ------------------------
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ClientMessage>(unsafe { torrent_client::CLIENT_OPTIONS.max_channel_size });
    let mut client = ClientHandle::new(tx);

    loop {
        tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Ctrl-C received, shutting down");
                break;
            },
            Ok(socket) = client_socket.accept() => {
                tracing::debug!("Terminal Client connection established");
                let pid = match socket.peer_pid() {
                    Ok(pid) => pid,
                    Err(e) => {
                        tracing::error!("Failed to get pid of Terminal Client: {}", e);
                        continue;
                    }
                };

                let mut terminal_client = TerminalClient{socket, pid};
                let message = match terminal_client.recv_message().await {
                    Ok(message) => message,
                    Err(e) => {
                        terminal_client_sockets.retain(|client| client.pid != terminal_client.pid);
                        tracing::error!("Failed to receive message from Terminal Client {}: {}", terminal_client.pid, e);
                        continue;
                    }
                };

                match message {
                    TerminalClientMessage::Shutdown => {
                        client.client_shutdown().await?;
                        tracing::info!("Received shutdown message in main loop, shutting down");
                        break;
                    },
                    TerminalClientMessage::AddTorrent{src, dst} => {
                        if !valid_src_and_dst(&src, &dst) {
                            if let Err(e) = terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::InvalidSrcOrDst }).await {
                                tracing::error!("Failed to send status message to Terminal Client {}: {}", terminal_client.pid, e);
                            }
                            tracing::error!("Invalid torrent file source path or destination received: {} {}", src, dst);
                        }
                        if let Err(e) = client.client_add_torrent(src, dst).await {
                            if let Err(e) = terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::InvalidSrcOrDst }).await {
                                tracing::error!("Failed to send status message to Terminal Client {}: {}", terminal_client.pid, e);
                            }
                            return Err(anyhow!("Failed to send an add message to client: {}", e));
                        }

                        if let Err(e) = terminal_client.send_message(&TerminalClientMessage::Status { exit_code: ExitCode::SUCCESS }).await {
                            tracing::error!("Failed to send status message to Terminal Client {}: {}", terminal_client.pid, e);
                        }
                    },
                    TerminalClientMessage::ListTorrents => {
                        if terminal_client_sockets.is_empty() {
                            client.client_list_torrents().await?;
                        }
                        terminal_client_sockets.push(terminal_client);
                    },
                    TerminalClientMessage::TerminalClientClosed => {
                        terminal_client_sockets.retain(|client| client.pid != terminal_client.pid);

                        if terminal_client_sockets.is_empty() {
                            client.client_terminal_closed().await?;
                        }
                    },
                    _ => {}
                }
                tracing::debug!("Terminal Client connection closed");
            },
            Some(client_message) = rx.recv() => {
                match client_message {
                    ClientMessage::TorrentsInfo{torrents} => {
                        let message = TerminalClientMessage::TorrentsInfo{torrents};

                        let mut clients_to_retain = Vec::new();
                        for terminal_client in terminal_client_sockets.iter_mut() {
                            if let Err(e) = terminal_client.send_message(&message).await {
                                clients_to_retain.push(terminal_client.pid);
                                tracing::error!("Failed to send torrents info message to Terminal Client {}: {}", terminal_client.pid, e);                                  
                            }
                        }

                        terminal_client_sockets.retain(|client| !clients_to_retain.contains(&client.pid));

                        if terminal_client_sockets.is_empty() {
                            client.client_terminal_closed().await?;
                        }
                    },
                    ClientMessage::Shutdown => {
                        tracing::info!("Received shutdown message in main loop, shutting down");
                        break;
                    },
                    _ => {
                        tracing::error!("Received invalid message from client");
                    }
                }
            },
        }
    }

    client.client_shutdown().await?;
    let _ = tokio::fs::remove_file(sock_path).await;

    client.join().await?;

    Ok(())
}