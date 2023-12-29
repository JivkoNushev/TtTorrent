use std::path::PathBuf;

use sha1::{Sha1, Digest};
use tokio::io::AsyncReadExt;
use getrandom::getrandom;
use anyhow::{Result, Context};

use crate::torrent::torrent_file::Sha1Hash;

pub trait UrlEncodable {
    fn as_url_encoded(&self) -> String;
}

impl UrlEncodable for [u8;20] {
    fn as_url_encoded(&self) -> String {
        percent_encoding::percent_encode(self, percent_encoding::NON_ALPHANUMERIC).to_string()
    }
}

pub trait AsBytes {
    fn as_bytes(&self) -> Vec<u8>;
}

pub fn sha1_hash(value: Vec<u8>) -> Sha1Hash {
    let mut hasher = Sha1::new();
    hasher.update(value);
    Sha1Hash::new(&hasher.finalize())
}   

pub async fn read_file_as_bytes(path: &str) -> Result<Vec<u8>> {
    // print current directory
    let mut buf = Vec::new();
    let mut file = tokio::fs::File::open(path).await.context("couldn't open file")?;

    file.read_to_end(&mut buf).await.context("couldn't read file")?;

    Ok(buf)
}

pub fn print_as_string(char_vec: &Vec<u8>) {
    println!("{}", char_vec.iter().map(|&c| c as char).collect::<String>());
}

pub fn rand_number_u32(min: u32, max: u32) -> u32 {
    let mut buffer = [0u8; 4]; // Assuming you want a u32 random number

    // Use getrandom to fill the buffer with random bytes
    if let Ok(_) = getrandom(&mut buffer) {
        // Convert the buffer to a u32 and get a random number within the desired range
        u32::from_ne_bytes(buffer) % (max - min) + min
    } else {
        panic!("Failed to generate random number");
    }
}