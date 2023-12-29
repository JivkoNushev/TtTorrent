use interprocess::local_socket::LocalSocketStream;

use std::io::Write;
use std::env::args;

use torrent_client::messager::ClientMessage;

fn main() {
    let mut client_socket = match LocalSocketStream::connect("/tmp/TtTClient.sock") {
        Ok(socket) => socket,
        Err(_) => {
            println!("Failed to connect to the client");
            return;
        }
    };

    let args = args().collect::<Vec<String>>();

    if args.len() < 2 {
        panic!("[Error] No command provided");
    }
    
    match args[1].as_str() {
        "download" => {
            if args.len() < 4 {
                panic!("[Error] Not enough arguments provided\nUsage: download <torrent_path> <dest_path>");
            }

            let mut torrent_path = args[2].clone();
            let mut dest_path = args[3].clone();
            
            if !torrent_path.starts_with("/") {
                let curr_path = std::env::current_dir().unwrap();
                let curr_path = curr_path.to_str().unwrap();
                torrent_path = format!("{}/{}", curr_path, torrent_path);
            }

            if !dest_path.starts_with("/") {
                let curr_path = std::env::current_dir().unwrap();
                let curr_path = curr_path.to_str().unwrap();
                dest_path = format!("{}/{}", curr_path, dest_path);
            }
            
            let message = ClientMessage::DownloadTorrent{src: torrent_path, dst: dest_path};

            let serialized_data = serde_json::to_string(&message).expect("Serialization failed");

            client_socket.write_all(serialized_data.as_bytes()).expect("Failed to send data");
        },
        "shutdown" => {
            let message = ClientMessage::Shutdown;

            let serialized_data = serde_json::to_string(&message).expect("Serialization failed");

            client_socket.write_all(serialized_data.as_bytes()).expect("Failed to send data");
        },
        "list" => {
            todo!("List torrents")
        }
        _ => {
            println!("Invalid command");
        }
    }
}