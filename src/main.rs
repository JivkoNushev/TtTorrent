mod torrent_file;
mod utils;

use crate::torrent_file::{
    TorrentFile, Info, FileInfo, 
    parse_torrent_file, 
    read_torrent_file_as_bytes
};
use crate::utils::read_bytes_as_string;

fn main() {
    // Read the torrent file into a byte array
    let torrent_data: Vec<u8> = match read_torrent_file_as_bytes("torrent_files/tfile2.torrent") {
        Ok(data) => data,
        Err(e) => {
            println!("Error reading torrent file: {:?}", e);
            return;
        }
    };

    // println!("{:?}", torrent_data);


    // Parse the torrent file

    let torrent_file: TorrentFile = match parse_torrent_file(&torrent_data) {
        Ok((_, torrent_file)) => torrent_file,
        Err(e) => {
            println!("Error parsing torrent file: {:?}", e);
            return;
        }
    };

    println!("{:?}", torrent_file);

}
