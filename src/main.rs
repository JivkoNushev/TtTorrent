mod torrent_file;
mod utils;
mod tracker;

use byteorder::BE;
use tokio::sync::mpsc;

use tracker::{get_peers};

use crate::{
    torrent_file:: {
        BencodedValue,
        parse_torrent_file, 
        read_file_as_bytes,
        parse_to_torrent_file,
    }, 
    tracker:: {
        Peer, 
        FileSaver,
    },
};

pub async fn download(torrent_file: BencodedValue, peers: Vec<Peer>, mut file_saver: FileSaver) {
    match tokio::join!(
        file_saver.start(),
        tokio::spawn(async move {
            for peer in peers {
                let torrent_file_clone = torrent_file.clone();
                tokio::spawn(async move {
                    peer.download(torrent_file_clone).await;
                });
            }
        }),
    ) {
        ((), Ok(_)) => println!("File downloaded successfully"),
        ((), Err(e)) => println!("Error downloading file: {:?}", e),
    };
}

#[tokio::main]
async fn main() {

    // Read the torrent file into a byte array
    let torrent_file: Vec<u8> = match read_file_as_bytes("torrent_files/centOS.torrent") {
        Ok(data) => data,
        Err(e) => {
            println!("Error reading torrent file: {:?}", e);
            return;
        }
    };

    // Parse the torrent file
    let mut torrent_file: BencodedValue = parse_torrent_file(&torrent_file);

    // Create a channel fot the file saver
    let (tx, rx) = mpsc::channel::<Vec<u8>>(100);

    // Create a file saver instance 
    let file_saver = FileSaver::new("newfile".to_string(), rx);

    // get the initial peers
    let peers = get_peers(&mut torrent_file, tx).await;
    // println!("{:?}", peers);
    
    download(torrent_file, peers, file_saver).await;
}
