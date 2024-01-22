use futures::{AsyncBufReadExt, AsyncWriteExt, AsyncReadExt};
use interprocess::local_socket::tokio::LocalSocketStream;
use anyhow::{anyhow, Result};


use std::env::args;
use std::path::PathBuf;
use std::process::{exit, Command, Stdio};

use torrent_client::messager::TerminalClientMessage;
use torrent_client::torrent::TorrentState;

pub struct TerminalClient {
    pub socket: LocalSocketStream,
    pub client_id: u32,
}

impl TerminalClient {
    pub async fn recv_message(&mut self) -> Result<TerminalClientMessage> {
        let mut reader = futures::io::BufReader::new(&mut self.socket);

        let mut buffer = String::new();
        let bytes_read = reader.read_line(&mut buffer).await?;

        if bytes_read == 0 {
            return Err(anyhow!("Failed to read message from socket"));
        }

        let message = serde_json::from_slice(buffer.as_bytes())?;
        Ok(message)
    }

    pub async fn recv_buffered_message(&mut self) -> Result<TerminalClientMessage> {
        let mut buffer = Vec::new();
        let bytes_read = self.socket.read_to_end(&mut buffer).await?;

        if bytes_read == 0 {
            return Err(anyhow!("Failed to read message from socket"));
        }

        let message = serde_json::from_slice(&buffer)?;
        Ok(message)
    }

    pub async fn send_message(&mut self, message: &TerminalClientMessage) -> Result<()> {
        let message = create_message(message);
        self.socket.write_all(&message).await?;

        Ok(())
    }

    pub async fn send_buffered_message(&mut self, message: &TerminalClientMessage) -> Result<()> {
        let message = serde_json::to_string(&message).expect("Serialization failed");
        self.socket.write_all(message.as_bytes()).await?;

        Ok(())
    }
}

fn create_message (message: &TerminalClientMessage) -> Vec<u8> {
    let mut serialized_data = serde_json::to_string(message).expect("Serialization failed");
    serialized_data.push('\n');
    serialized_data.as_bytes().to_vec()
}

async fn create_client_socket() -> LocalSocketStream {
    let client_socket = match LocalSocketStream::connect(torrent_client::SOCKET_PATH).await {
        Ok(socket) => socket,
        Err(e) => {
            eprintln!("[Error] Failed to connect to the client: {}", e);
            exit(1);
        }
    };

    client_socket
}

fn check_file(path: &PathBuf) -> bool {
    path.exists() && path.is_file()
}

fn check_dir(path: &PathBuf) -> bool {
    std::fs::create_dir_all(path).is_ok()
}

fn check_download_arguments(torrent_path: &PathBuf, dest_path: &PathBuf) -> Result<()> {
    if !check_file(torrent_path) {
        return Err(anyhow!("Invalid torrent file path"));
    }

    if !check_dir(dest_path) {
        return Err(anyhow!("Invalid destination path"));
    }

    Ok(())
}

fn calculate_percentage(pieces_count: usize, pieces_left: usize) -> f64 {
    let pieces_count = pieces_count as f64;
    let pieces_left = pieces_left as f64;

    let percentage = (pieces_count - pieces_left) / pieces_count * 100.0;
    (percentage * 100.0).round() / 100.0
}

fn print_torrent_infos(torrents: Vec<TorrentState>) {
    print!("{}[2J", 27 as char);

    if torrents.is_empty() {
        println!("No torrents");
        return;
    }
    println!(
        "{0: <20} | {1: <20} | {2: <20} | {3: <20}",
        "name", "progress", "downloaded", "peers"
    );
    println!("{}", "-".repeat(89));

    for torrent in torrents {
        let downloaded_percentage = calculate_percentage(torrent.blocks_count, torrent.needed_blocks.len());
        let peers = torrent.peers.len();

        println!(
            "{0: <20} | {1: <20} | {2: <20} | {3: <20}", 
            torrent.torrent_name, downloaded_percentage, torrent.downloaded, peers
        );
    }
}

fn print_help_menu() {
    println!("
Usage: 
    
        torrent_client <command> [args]

Commands:

        start - Start the client daemon

        stop - Stop the client daemon

        download <torrent_path> <dest_path> - Download a torrent file

        shutdown - Shutdown the client

        list - List all torrents


        --help - Print this help menu
"
    );
}

async fn download(mut client: TerminalClient, src: &str, dest: &str) -> Result<()> {
    let mut torrent_path = PathBuf::from(src);
    let mut dest_path = PathBuf::from(dest);

    check_download_arguments(&mut torrent_path, &mut dest_path)?;
    let torrent_path = PathBuf::from(torrent_path).canonicalize()?;
    let dest_path = PathBuf::from(dest_path).canonicalize()?;

    let torrent_path = torrent_path.to_str().unwrap().to_string();
    let dest_path = dest_path.to_str().unwrap().to_string();
    
    client.send_message(&TerminalClientMessage::Download{src: torrent_path, dst: dest_path}).await?;

    match client.recv_message().await? {
        TerminalClientMessage::Status{exit_code} => {
            match exit_code {
                torrent_client::messager::ExitCode::SUCCESS => {
                    println!("Download started");
                },
                torrent_client::messager::ExitCode::InvalidSrcOrDst => {
                    return Err(anyhow!("Invalid src or dst"));
                }
            }
        },
        _ => {
            return Err(anyhow!("Received invalid message from client"));
        }
    };

    Ok(())
}

async fn list_torrents(mut torrent_client: TerminalClient) -> Result<()> {
    println!("No torrent states...");
    loop {
        tokio::select! {
            message = torrent_client.recv_message() => {
                match message? {
                    TerminalClientMessage::TorrentsInfo{torrents} => {
                        print_torrent_infos(torrents);
                    },
                    _ => {
                        return Err(anyhow!("Received invalid message from client"));
                    }
                }
            },
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = args().collect::<Vec<String>>();

    if args.len() < 2 {
        eprintln!("[Error] No command provided");
        println!("Usage: tttorrent <command> [args]\
                \nFor list of commands use: torrent_client --help");

        exit(1);
    }

    if args.contains(&"--help".to_string()) {
        print_help_menu();
        exit(0);
    }

    // if command is start, start the daemon process of the client
    if args[1] == "start" {
        // TODO: doesn't work on unix

        // start target\debug\tttorrent-daemon.exe not as a child process but as a daemon
        let path = format!("{}/target/debug/tttorrent-daemon.exe", std::env::current_dir().unwrap().to_str().unwrap());
        let daemon_path = std::path::Path::new(&path);

        let _ = Command::new(daemon_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

        exit(0);
    }

    let mut terminal_client = TerminalClient{socket: create_client_socket().await, client_id: std::process::id()};

    match args[1].as_str() {
        "download" => {
            if args.len() < 4 || args.len() > 4 {
                eprintln!("[Error] Invalid number of arguments provided");
                println!("Usage: tttorrent download <torrent_path> <dest_path>");
                exit(1);
            }

            if let Err(e) = download(terminal_client, &args[2], &args[3]).await {
                eprintln!("Failed to download torrent: {}", e);
                exit(1);
            }
        },
        "shutdown" => {
            if args.len() != 2 {
                eprintln!("[Error] Invalid number of arguments provided");
                println!("Usage: tttorrent shutdown");

                exit(1);
            }

            if let Err(e) = terminal_client.send_message(&TerminalClientMessage::Shutdown).await {
                eprintln!("Failed to send shutdown message to client: {}", e);
                exit(1);
            }

            println!("Shutting down client daemon...")
        },
        "list" => {
            if args.len() != 2 {
                eprintln!("[Error] Invalid number of arguments provided");
                println!("Usage: tttorrent list");

                exit(1);
            }

            if let Err(e) = terminal_client.send_message(&TerminalClientMessage::ListTorrents{client_id: terminal_client.client_id}).await {
                eprintln!("Failed to send list torrents message to client: {}", e);
                exit(1);
            }

            if let Err(e) = list_torrents(terminal_client).await {
                eprintln!("Failed to list torrents: {}", e);
                exit(1);
            }
        },
        _ => {
            eprintln!("[Error] Invalid command");
            exit(1);
        }
    }
}