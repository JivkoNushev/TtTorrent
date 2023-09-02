mod parser;
mod utils;

pub use parser::parse_torrent_file;
pub use utils::read_torrent_file_as_bytes;

pub enum TorrentFile {
    Info(Info),
    Announce(String),
} 

pub struct Info {
    
}
