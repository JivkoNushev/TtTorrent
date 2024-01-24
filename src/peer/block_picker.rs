use serde::{Serialize, Deserialize};
use anyhow::Result;
use tokio_stream::StreamExt;

use std::sync::Arc;

use crate::torrent::torrent_info::TorrentInfo;
use crate::utils::rand_number_u32;

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockPickerState {
    pub needed_blocks: Vec<usize>,
    pub needed_pieces: Vec<usize>,
    pub peer_pieces: Vec<usize>,

    pub torrent_info: TorrentInfo,  
}

impl BlockPickerState {
    pub fn from_context(block_picker: BlockPicker) -> BlockPickerState {
        BlockPickerState {
            needed_blocks: block_picker.needed_blocks,
            needed_pieces: block_picker.needed_pieces,
            peer_pieces: Vec::new(),

            torrent_info: (*block_picker.torrent_info).clone(),
        }
    }
}


#[derive(Debug, Clone)]
pub struct BlockPicker {
    pub needed_blocks: Vec<usize>,
    pub needed_pieces: Vec<usize>,

    pub torrent_info: Arc<TorrentInfo>,    
}

impl BlockPicker {
    pub fn new(needed_blocks: Vec<usize>, needed_pieces: Vec<usize>, torrent_info: Arc<TorrentInfo>) -> BlockPicker {
        BlockPicker {
            needed_blocks,
            needed_pieces,

            torrent_info, 
        }
    }

    pub fn from_state(block_picker_state: BlockPickerState) -> BlockPicker {
        BlockPicker {
            needed_blocks: block_picker_state.needed_blocks,
            needed_pieces: block_picker_state.needed_pieces,

            torrent_info: Arc::new(block_picker_state.torrent_info),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.needed_blocks.is_empty()
    }

    pub async fn pick_random(&mut self, peer_pieces: &Vec<usize>) -> Result<Option<usize>> {
        if self.is_empty() {
            return Ok(None);
        }

        let common_pieces = peer_pieces
            .iter()
            .filter(|&i| self.needed_pieces.contains(i))
            .collect::<Vec<&usize>>();

        if common_pieces.is_empty() {
            return Ok(None);
        }

        let random_piece_index = match rand_number_u32(0, common_pieces.len() as u32) {
            Ok(random_piece) => random_piece as usize,
            Err(_) => {
                eprintln!("[Error] Failed to generate random number");
                return Ok(None);
            }
        };

        let random_piece = common_pieces[random_piece_index];

        let current_piece_length = self.torrent_info.get_specific_piece_length(*random_piece);
        let blocks_in_current_piece = current_piece_length.div_ceil(crate::BLOCK_SIZE);

        let piece_blocks = {
            let mut piece_blocks = Vec::new();

            let mut blocks_iter = tokio_stream::iter(0..blocks_in_current_piece);
            while let Some(i) = blocks_iter.next().await {
                let block = *random_piece * self.torrent_info.blocks_in_piece + i;
                if self.needed_blocks.contains(&block) {
                    piece_blocks.push(block);
                }
            }

            piece_blocks
        };

        if piece_blocks.is_empty() {
            eprintln!("[Error] peer has no blocks in piece {random_piece}");
            return Ok(None);
        }

        let random_block_index = match rand_number_u32(0, piece_blocks.len() as u32) {
            Ok(random_block) => random_block as usize,
            Err(_) => {
                eprintln!("Failed to generate random number");
                return Ok(None);
            }
        };

        // if needed_blocks_guard.len() > crate::BLOCK_REQUEST_COUNT {
            self.needed_blocks.retain(|&i| i != piece_blocks[random_block_index]);

            let blocks = (random_piece * self.torrent_info.blocks_in_piece..random_piece * self.torrent_info.blocks_in_piece + blocks_in_current_piece).collect::<Vec<usize>>();
            let retain_piece = blocks.iter().all(|&i| !self.needed_blocks.contains(&i));
            if retain_piece {
                self.needed_pieces.retain(|&i| i != *random_piece);
            }
        // }

        Ok(Some(piece_blocks[random_block_index]))
    }
}