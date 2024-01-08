use sha1::{Sha1, Digest};
use tokio::io::AsyncReadExt;
use getrandom::getrandom;
use anyhow::{anyhow, Result, Context};

use crate::torrent::torrent_file::Sha1Hash;
use crate::messager::TerminalClientMessage;

pub struct CommunicationPipe {
    pub tx: tokio::sync::mpsc::Sender<crate::messager::ClientMessage>,
    pub rx: tokio::sync::mpsc::Receiver<crate::messager::ClientMessage>,
}

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
    let hash = hasher.finalize();
    let hash = hash.as_slice().try_into().unwrap(); // sha1 hash is always 20 bytes in this case
    Sha1Hash::new(hash)
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

pub fn rand_number_u32(min: u32, max: u32) -> Result<u32> {
    let mut buffer = [0u8; 4];
    if let Err(e) = getrandom(&mut buffer) {
        return Err(anyhow!("Failed to generate random number: {}", e));
    }
    Ok(u32::from_ne_bytes(buffer) % (max - min) + min)
}

pub fn valid_src_and_dst(src: &str, dst: &str) -> bool {
    let torrent_file = std::path::Path::new(src);
    let directory = std::path::Path::new(dst);
    if  !torrent_file.exists()                              || 
        !torrent_file.is_file()                             || 
        torrent_file.extension().is_none()               ||
        "torrent" != torrent_file.extension().unwrap()
    {
        return false;
    }

    match std::fs::create_dir_all(directory) {
        Ok(_) => true,
        Err(_) => false
    }
}

pub fn create_message (message: &TerminalClientMessage) -> Vec<u8> {
    let mut serialized_data = serde_json::to_string(message).expect("Serialization failed");
    serialized_data.push('\n');
    serialized_data.as_bytes().to_vec()
}