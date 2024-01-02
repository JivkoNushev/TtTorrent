use interprocess::local_socket::LocalSocketStream;

use std::io::{Read, Write};
use std::env::args;
use std::process::{exit, Command, Stdio};

use torrent_client::messager::TerminalClientMessage;

fn check_file(path: &str) -> bool {
    let file = std::path::Path::new(path);
    file.exists() && file.is_file()
}

fn check_dir(path: &str) -> bool {
    let dir = std::path::Path::new(path);
    std::fs::create_dir_all(dir).is_ok()
}

fn calculate_percentage(pieces_count: usize, pieces_left: usize) -> f64 {
    let pieces_count = pieces_count as f64;
    let pieces_left = pieces_left as f64;

    let percentage = (pieces_count - pieces_left) / pieces_count * 100.0;
    percentage
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

fn list_torrents(socket: &mut LocalSocketStream) {
    let (tx, rx) = std::sync::mpsc::channel::<()>();

    ctrlc::set_handler(move || {
        tx.send(()).expect("Failed to send ctrl-c signal");
    }).expect("Failed to set ctrl+c handler");

    loop {
        // check if ctrl+c was pressed
        if let Ok(_) = rx.try_recv() {
            println!("Shutting down client daemon...");

            let mut serialized_data = serde_json::to_string(&TerminalClientMessage::TerminalClientClosed).expect("Serialization failed");
            serialized_data.push('\n');
            let _ = socket.write(serialized_data.as_bytes());
            break;
        }
        
        let mut message = Vec::new();
        if let Err(e) = socket.read_to_end(&mut message) {
            if e.kind() != std::io::ErrorKind::WouldBlock {
                eprintln!("Failed to read message from local socket: {}", e);
            }
    
            continue;
        }

        if message.is_empty() {
            continue;
        }

        println!("received message: {:?}", message);
        
        let message = match serde_json::from_slice::<TerminalClientMessage>(&message) {
            Ok(message) => message,
            Err(e) => {
                if e.is_eof() {
                    println!("Client daemon stopped running");
                    break;
                }

                eprintln!("Failed to deserialize message from local socket: {}", e);
                continue;
            }
        };
        
        let torrent_states;
        match message {
            TerminalClientMessage::TorrentsInfo{torrents} => {
                torrent_states = torrents;
            },
            _ => {
                eprintln!("Invalid message");
                exit(1);
            }
        }
    
        for torrent_state in torrent_states {
            let downloaded_percentage = calculate_percentage(torrent_state.pieces_count, torrent_state.pieces.len());
            println!("Name: {}\nProgress: {}%\nPeers: {:?}\n", torrent_state.torrent_name, downloaded_percentage, torrent_state.peers);
        }
    }
}

#[tokio::main]
async fn main() {
    let args = args().collect::<Vec<String>>();

    if args.len() < 2 {
        eprintln!("[Error] No command provided");
        println!("Usage: torrent_client <command> [args]\
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

    let mut client_socket = match LocalSocketStream::connect(torrent_client::SOCKET_PATH) {
        Ok(socket) => socket,
        Err(_) => {
            eprintln!("[Error] Failed to connect to the client");
            exit(1);
        }
    };
    client_socket.set_nonblocking(true).expect("Failed to set nonblocking mode");
    
    match args[1].as_str() {
        "download" => {
            if args.len() < 4 {
                eprintln!("[Error] Not enough arguments provided");
                println!("Usage: torrent_client download <torrent_path> <dest_path>");

                exit(1);
            }

            let mut torrent_path = args[2].clone();
            let mut dest_path = args[3].clone();

            if !check_file(&torrent_path) {
                eprintln!("[Error] Torrent file does not exist");
                println!("Usage: torrent_client download <torrent_path> <dest_path>");

                exit(1);
            }
            if !check_dir(&dest_path) {
                eprintln!("[Error] Destination directory does not exist");
                println!("Usage: torrent_client download <torrent_path> <dest_path>");

                exit(1);
            }
            
            if !torrent_path.starts_with("/") {
                let curr_path = match std::env::current_dir() {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("[Error] Failed to get current directory: {e}");
                        exit(1);
                    }
                };
                let curr_path = curr_path.to_str().unwrap(); // paths are always valid utf8
                torrent_path = format!("{}/{}", curr_path, torrent_path);
            }

            if !dest_path.starts_with("/") {
                let curr_path = match std::env::current_dir() {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("[Error] Failed to get current directory: {e}");
                        exit(1);
                    }
                };
                let curr_path = curr_path.to_str().unwrap(); // // paths are always valid utf8
                dest_path = format!("{}/{}", curr_path, dest_path);
            }
            
            let message = TerminalClientMessage::Download{src: torrent_path, dst: dest_path};

            let mut serialized_data = serde_json::to_string(&message).expect("Serialization failed");
            serialized_data.push('\n');
            client_socket.write(serialized_data.as_bytes()).expect("Failed to send data");
        },
        "shutdown" => {
            let mut serialized_data = serde_json::to_string(&TerminalClientMessage::Shutdown).expect("Serialization failed");
            serialized_data.push('\n');
            client_socket.write(serialized_data.as_bytes()).expect("Failed to send data");
        },
        "list" => {
            let mut serialized_data = serde_json::to_string(&TerminalClientMessage::ListTorrents).expect("Serialization failed");
            serialized_data.push('\n');
            println!("{}", serialized_data);
            let _ = client_socket.write(serialized_data.as_bytes()).expect("Failed to send data");

            list_torrents(&mut client_socket);
        },
        _ => {
            eprintln!("[Error] Invalid command");
            exit(1);
        }
    }
}