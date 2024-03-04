use anyhow::{anyhow, Result};

use std::env::args;
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};

use torrent_client::messager::TerminalClientMessage;
use torrent_client::torrent::TorrentState;
use torrent_client::utils::terminal::{TerminalClient, create_client_socket};

fn check_file(path: &Path) -> bool {
    path.exists() && path.is_file()
}

fn check_dir(path: &PathBuf) -> bool {
    std::fs::create_dir_all(path).is_ok()
}

fn check_add_arguments(torrent_path: &Path, dest_path: &PathBuf) -> Result<()> {
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
    // print!("{}[2J", 27 as char);
    print!("{esc}c", esc = 27 as char);

    if torrents.is_empty() {
        println!("No torrents");
        return;
    }
    println!(
        "{0: <20} | {1: <20}  | {2: <20}   | {3: <20}   | {4: <20}",
        "name", "progress", "downloaded", "uploaded", "peers"
    );
    println!("{}", "-".repeat(109));

    for torrent in torrents {
        let downloaded_percentage = calculate_percentage(torrent.torrent_info.pieces_count, torrent.needed.pieces.len());
        let peers = torrent.peers.len();

        println!(
            "{0: <20} | {1: <20}% | {2: <20}KB | {3: <20}KB | {4: <20}", 
            torrent.torrent_name, downloaded_percentage, torrent.downloaded / 1000, torrent.uploaded / 1000, peers
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

        add <torrent_path> <dest_path> - add a torrent file

        list - List all torrents


        --help - Print this help menu
"
    );
}

async fn add(mut client: TerminalClient, src: &str, dest: &str) -> Result<()> {
    let torrent_path = PathBuf::from(src);
    let dest_path = PathBuf::from(dest);

    check_add_arguments(&torrent_path, &dest_path)?;
    let torrent_path = torrent_path.canonicalize()?;
    let dest_path = dest_path.canonicalize()?;

    let torrent_path = torrent_path.to_str().unwrap().to_string();
    let dest_path = dest_path.to_str().unwrap().to_string();
    
    client.send_message(&TerminalClientMessage::AddTorrent{src: torrent_path, dst: dest_path}).await?;

    match client.recv_message().await? {
        TerminalClientMessage::Status{exit_code} => {
            match exit_code {
                torrent_client::utils::ExitCode::SUCCESS => {
                    println!("Torrent added");
                },
                torrent_client::utils::ExitCode::InvalidSrcOrDst => {
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
        let path = format!("{}/target/release/tttorrent-client", std::env::current_dir().unwrap().to_str().unwrap());
        let daemon_path = std::path::Path::new(&path);

        let args_ = &args[2..];

        let _ = Command::new(daemon_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .args(args_)
            .spawn();

        exit(0);
    }

    let mut terminal_client = TerminalClient{socket: create_client_socket().await, pid: std::process::id()};

    match args[1].as_str() {
        "add" => {
            if args.len() < 4 || args.len() > 4 {
                eprintln!("[Error] Invalid number of arguments provided");
                println!("Usage: tttorrent add <torrent_path> <dest_path>");
                exit(1);
            }

            if let Err(e) = add(terminal_client, &args[2], &args[3]).await {
                eprintln!("Failed to add torrent: {}", e);
                exit(1);
            }
        },
        "list" => {
            if args.len() != 2 {
                eprintln!("[Error] Invalid number of arguments provided");
                println!("Usage: tttorrent list");

                exit(1);
            }

            if let Err(e) = terminal_client.send_message(&TerminalClientMessage::ListTorrents).await {
                eprintln!("Failed to send list torrents message to client: {}", e);
                exit(1);
            }

            if let Err(e) = list_torrents(terminal_client).await {
                eprintln!("Failed to list torrents: {}", e);
                exit(1);
            }
        },
        "stop" => {
            if args.len() != 2 {
                eprintln!("[Error] Invalid number of arguments provided");
                println!("Usage: tttorrent stop");

                exit(1);
            }

            while terminal_client.send_message(&TerminalClientMessage::Shutdown).await.is_ok() {}
            
            println!("Stopping client daemon...");
        },
        _ => {
            eprintln!("[Error] Invalid command");
            exit(1);
        }
    }
}