use crate::torrent_file::{TorrentFile, Info};

use std::io::{Result, Read};

pub fn read_torrent_file_as_bytes(path: &str) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut file = std::fs::File::open(path)?;

    file.read_to_end(&mut buf)?;

    Ok(buf)
}