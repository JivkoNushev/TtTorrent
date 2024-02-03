use serde::{Serialize, Deserialize};

use crate::torrent::TorrentInfo;

use super::{BlockPicker, Piece};


#[derive(Debug, Serialize, Deserialize)]
pub struct BlockPickerState {
    pub pieces: Vec<Piece>,
    pub torrent_info: TorrentInfo,  
}

impl BlockPickerState {
    pub fn from_context(block_picker: BlockPicker) -> BlockPickerState {
        BlockPickerState {
            pieces: block_picker.pieces,
            torrent_info: (*block_picker.torrent_info).clone(),
        }
    }
}