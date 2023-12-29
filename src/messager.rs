use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    DownloadTorrent{src: String, dst: String},
    Shutdown,
    DownloadedPiece{piece_index: usize, piece: Vec<u8>},
}