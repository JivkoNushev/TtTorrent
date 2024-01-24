use anyhow::Result;
use serde::{Serialize, Deserialize};

use super::TorrentFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentInfo {
    pub pieces_count: usize,
    pub blocks_count: usize,
    pub torrent_size: u64,
    pub piece_length: usize,
    pub block_length: usize,
    pub blocks_in_piece: usize,
}

impl TorrentInfo {
    pub fn new(torrent_file: &TorrentFile) -> Result<TorrentInfo> {
        let pieces_count = torrent_file.get_pieces_count()?;
        let blocks_count = torrent_file.get_blocks_count()?;
        let torrent_size = torrent_file.get_torrent_length()?;
        let piece_length = torrent_file.get_piece_length()?;
        let block_length = torrent_file.get_block_length()?;
        let blocks_in_piece = torrent_file.get_blocks_in_piece()?;

        Ok(TorrentInfo {
            pieces_count,
            blocks_count,
            torrent_size,
            piece_length, 
            block_length,
            blocks_in_piece,
        })
    }

    pub fn get_specific_piece_length(&self, index: usize) -> usize {
        if self.pieces_count - 1 != index {
            return self.piece_length;
        }

        let length = self.torrent_size % self.piece_length as u64;

        if length > 0 { length as usize } else { self.piece_length }
    }

    pub fn get_specific_block_length(&self, index: usize) -> usize {
        if self.blocks_count - 1 != index {
            return self.block_length;
        }

        let length = self.torrent_size % self.block_length as u64;

        if length > 0 { length as usize } else { self.block_length }
    }
}