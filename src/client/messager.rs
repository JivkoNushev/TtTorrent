
#[derive(Debug, Clone)]
pub enum MessageType {
    DownloadTorrent,
    DownloadedPiecesCount,
}   

#[derive(Debug, Clone)]
pub struct InterProcessMessage {
    pub size: usize,
    pub message_type: MessageType,
    pub torrent_name: String,
    pub payload: Vec<u8>,
}

impl InterProcessMessage {
    pub fn new(message_type: MessageType, torrent_name: String, payload: Vec<u8>) -> InterProcessMessage {
        let size = payload.len() + 1;

        InterProcessMessage {
            size,
            message_type,
            torrent_name,
            payload,
        }
    }
}