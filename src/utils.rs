use tokio::io::AsyncReadExt;
use getrandom::getrandom;
use anyhow::{Result, Context};

pub mod sha1hash;
pub use sha1hash::Sha1Hash;

pub mod bencode;
pub use bencode::BencodedValue;


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

impl UrlEncodable for Vec<u8> {
    fn as_url_encoded(&self) -> String {
        percent_encoding::percent_encode(self, percent_encoding::NON_ALPHANUMERIC).to_string()
    }
}

pub async fn read_file_as_bytes(path: &std::path::Path) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut file = tokio::fs::File::open(path).await.context("couldn't open file")?;

    file.read_to_end(&mut buf).await.context("couldn't read file")?;

    Ok(buf)
}

pub fn print_as_string(char_vec: &Vec<u8>) {
    println!("{}", char_vec.iter().map(|&c| c as char).collect::<String>());
}

pub fn valid_src_and_dst(src: &str, dst: &str) -> bool {
    let torrent_file = std::path::Path::new(src);
    let directory = std::path::Path::new(dst);
    if  !torrent_file.exists()                              || 
        !torrent_file.is_file()                             || 
        torrent_file.extension().is_none()                  ||
        "torrent" != torrent_file.extension().unwrap()
    {
        return false;
    }

    match std::fs::create_dir_all(directory) {
        Ok(_) => true,
        Err(_) => false
    }
}

pub fn is_zero_aligned(buf: &[u8]) -> bool {
    let (prefix, aligned, suffix) = unsafe { buf.align_to::<u128>() };

    prefix.iter().all(|&x| x == 0)
        && suffix.iter().all(|&x| x == 0)
        && aligned.iter().all(|&x| x == 0)
}

pub fn generate_random_client_id() -> [u8; 20] {
    let mut client_id = [0u8; 20];
    client_id[0..10].copy_from_slice(b"TtT-1-0-0-");
    getrandom(&mut client_id[10..]).expect("Failed to generate random client id");

    tracing::debug!("Generated client id: {:?}", client_id);

    client_id
}

pub fn print_bencoded_value(bencoded_value: &BencodedValue) {
    match bencoded_value {
        BencodedValue::ByteString(byte_string) => {
            print!("Byte String: ");
            print_as_string(byte_string);
        },
        BencodedValue::Integer(byte_integer) => {
            println!("Byte Integer: {}", byte_integer);
        },
        BencodedValue::List(byte_list) => {
            println!("Byte List: ");
            for byte_value in byte_list {
                print_bencoded_value(byte_value);
            }
        },
        BencodedValue::Dict(byte_dict) => {
            println!("Byte Dict: ");
            for (key, value) in byte_dict {
                print!("Key: ");
                print_as_string(key);
                print!("Value: ");
                print_bencoded_value(value);
            }
        },
        BencodedValue::ByteSha1Hashes(byte_sha1_hashes) => {
            println!("Byte Sha1 Hashes: ");
            for byte_sha1_hash in byte_sha1_hashes {
                print!("Byte Sha1 Hash: ");
                print_as_string(&byte_sha1_hash.as_bytes().to_vec());
            }
        },
        _ => {
            println!("Invalid bencoded value");
        }
    }
}