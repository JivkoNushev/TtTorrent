use interprocess::local_socket::LocalSocketStream;

use std::io::Write;
use std::env::args;
use std::process::{exit, Command, Stdio};

use torrent_client::messager::ClientMessage;

fn check_file(path: &str) -> bool {
    let file = std::path::Path::new(path);
    file.exists() && file.is_file()
}

fn check_dir(path: &str) -> bool {
    let dir = std::path::Path::new(path);
    std::fs::create_dir_all(dir).is_ok()
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

fn main() {
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
            
            let message = ClientMessage::DownloadTorrent{src: torrent_path, dst: dest_path};

            let serialized_data = serde_json::to_string(&message).expect("Serialization failed");
            client_socket.write_all(serialized_data.as_bytes()).expect("Failed to send data");
        },
        "shutdown" => {
            let serialized_data = serde_json::to_string(&ClientMessage::Shutdown).expect("Serialization failed");
            client_socket.write_all(serialized_data.as_bytes()).expect("Failed to send data");
        },
        "list" => {
            todo!("List torrents")
        }
        _ => {
            eprintln!("[Error] Invalid command");
            exit(1);
        }
    }
}