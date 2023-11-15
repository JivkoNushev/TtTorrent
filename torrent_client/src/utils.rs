use std::io::{ Read, Result };
use sha1::{Sha1, Digest};

use crate::torrent::torrent_file::Sha1Hash;


pub trait UrlEncodable {
    fn as_url_encoded(&self) -> String;
}

impl UrlEncodable for [u8;20] {
    fn as_url_encoded(&self) -> String {
        percent_encoding::percent_encode(self, percent_encoding::NON_ALPHANUMERIC).to_string()
    }

}

pub fn sha1_hash(value: Vec<u8>) -> Sha1Hash {
    let mut hasher = Sha1::new();
    hasher.update(value);
    Sha1Hash::new(&hasher.finalize())
}   

pub fn read_file_as_bytes(path: &str) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut file = std::fs::File::open(path)?;

    file.read_to_end(&mut buf)?;

    Ok(buf)
}

pub fn print_as_string(char_vec: &Vec<u8>) {
    println!("{}", char_vec.iter().map(|&c| c as char).collect::<String>());
}