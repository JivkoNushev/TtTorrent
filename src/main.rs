mod torrent_file;
mod utils;
mod tracker;

use tracker::get_peers;

use crate::torrent_file::{
    BencodedValue,
    parse_torrent_file, 
    read_torrent_file_as_bytes
};

fn main() {

    // Read the torrent file into a byte array
    let torrent_data: Vec<u8> = match read_torrent_file_as_bytes("torrent_files/centOS.torrent") {
        Ok(data) => data,
        Err(e) => {
            println!("Error reading torrent file: {:?}", e);
            return;
        }
    };

    // println!("{:?}", torrent_data);


    // Parse the torrent file

    let mut torrent_file: BencodedValue = parse_torrent_file(&torrent_data);

    println!("{:?}", torrent_file);


    // Get the tracker info
    get_peers(&mut torrent_file);
}
