mod parser;
mod utils;

pub use parser::parse_torrent_file;
pub use utils::read_torrent_file_as_bytes;

pub enum TorrentFile {
    Announce(String),
    Info(Info),
} 

pub struct Info {
    
}

impl TorrentFile {
    pub fn new() -> TorrentFile {
        TorrentFile::Info(Info {})
    }
}
    
