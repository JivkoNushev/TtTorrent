use serde::{Serialize, Deserialize};

use crate::peer::{PeerAddress, BlockPickerState};

use super::{TorrentInfo, TorrentContext};


#[derive(Debug, Serialize, Deserialize)]
pub struct TorrentState {
    pub src_path: String,
    pub dest_path: String,
    pub torrent_name: String,
    pub needed: BlockPickerState,
    pub bitfield: Vec<u8>,
    pub peers: Vec<PeerAddress>,

    pub torrent_info: TorrentInfo,

    pub downloaded: u64,
    pub uploaded: u64,
}

impl TorrentState {
    pub async fn new(torrent_context: TorrentContext) -> Self {
        let needed = BlockPickerState::from_context(torrent_context.needed.lock().await.clone());
        Self {
            src_path: torrent_context.src_path,
            dest_path: torrent_context.dest_path,
            torrent_name: torrent_context.torrent_name,
            needed,
            bitfield: torrent_context.bitfield.lock().await.clone(),
            peers: torrent_context.peers,

            torrent_info: (*torrent_context.torrent_info).clone(),
            downloaded: torrent_context.downloaded.lock().await.clone(),
            uploaded: torrent_context.uploaded.lock().await.clone(),
        }
    }
}