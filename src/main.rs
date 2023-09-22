mod torrent_file;
mod utils;

use std::collections::HashMap;

use crate::torrent_file::{
    BencodedValue,
    parse_torrent_file, 
    read_torrent_file_as_bytes
};
use crate::utils::print_bytes_as_string;


fn main() {

    // Read the torrent file into a byte array
    let torrent_data: Vec<u8> = match read_torrent_file_as_bytes("torrent_files/ReDHaT.torrent") {
        Ok(data) => data,
        Err(e) => {
            println!("Error reading torrent file: {:?}", e);
            return;
        }
    };

    // println!("{:?}", torrent_data);


    // Parse the torrent file

    let torrent_file: BencodedValue = parse_torrent_file(&torrent_data);

    println!("{:?}", torrent_file);

}
