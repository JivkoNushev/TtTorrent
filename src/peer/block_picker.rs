use serde::{Serialize, Deserialize};
use anyhow::Result;

use std::sync::Arc;

use crate::torrent::torrent_info::TorrentInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Piece {
    pub index: u32,
    pub block_count: usize,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub index: u32,
    pub begin: u32,
    pub length: u32,

    pub number: usize,
    pub data: Option<Vec<u8>>,
}

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


#[derive(Debug, Clone)]
pub struct BlockPicker {
    pub pieces: Vec<Piece>,
    pub torrent_info: Arc<TorrentInfo>,    
}

impl BlockPicker {
    pub fn new(pieces: Vec<Piece>, torrent_info: Arc<TorrentInfo>) -> BlockPicker {
        BlockPicker {
            pieces,
            torrent_info, 
        }
    }

    pub fn from_state(block_picker_state: BlockPickerState) -> BlockPicker {
        BlockPicker {
            pieces: block_picker_state.pieces,
            torrent_info: Arc::new(block_picker_state.torrent_info),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    pub fn contains(&self, piece_index: u32) -> bool {
        self.pieces.iter().any(|p| p.index == piece_index)
    }

    pub fn block_count(&self) -> usize {
        self.pieces.iter().map(|p| p.block_count).sum()
    }

    pub async fn pick_random(&mut self, peer_bitfield: &Vec<u8>) -> Result<Option<Block>> {
        if self.is_empty() {
            tracing::warn!("Trying to pick a random block from an empty block picker.");
            return Ok(None);
        }

        let block = 
        if let Some(piece) = self.pieces.iter_mut().find(|p| {tracing::trace!("piece {p:?} - {}", 0 < peer_bitfield[p.index as usize / 8 ] & 1 << (7 - p.index % 8)); 0 < peer_bitfield[p.index as usize / 8 ] & 1 << (7 - p.index % 8) }) {
            piece.block_count -= 1;

            let block_number = piece.index as usize * self.torrent_info.blocks_in_piece + piece.block_count;
            let block = Block {
                index: piece.index,
                begin: (piece.block_count * self.torrent_info.block_length) as u32,
                length: self.torrent_info.get_specific_block_length(block_number as u32) as u32,

                number: block_number,
                data: None,
            };

            block
        }
        else {
            tracing::trace!("No piece found in block picker.");
            return Ok(None);
        };

        if block.begin == 0 {
            tracing::trace!("Removing piece {} from block picker.", block.index);
            self.pieces.retain(|p| p.index != block.index);
        }

        tracing::trace!("Picked block: {:?}", block);
        Ok(Some(block))
    }

    pub async fn get_end_game_blocks(&mut self, peer_bitfield: &Vec<u8>) -> Result<Option<Vec<Block>>> {
        if self.is_empty() {
            tracing::warn!("Trying to pick a random block from an empty block picker.");
            return Ok(None);
        }

        let mut blocks = Vec::new();
        for piece in self.pieces.iter() {
            if 0 < peer_bitfield[piece.index as usize / 8 ] & 1 << (7 - piece.index % 8) {
                for n in 0..piece.block_count {
                    let block_number = piece.index as usize * self.torrent_info.blocks_in_piece + n;
                    let block = Block {
                        index: piece.index,
                        begin: (n * self.torrent_info.block_length) as u32,
                        length: self.torrent_info.get_specific_block_length(block_number as u32) as u32,

                        number: block_number,
                        data: None,
                    };

                    blocks.push(block);
                }
            }
        }

        Ok(Some(blocks))
    }
}