use anyhow::{anyhow, Result, Context};
use serde::{Serialize, Deserialize};

use crate::utils::{read_file_as_bytes, sha1_hash};
use crate::utils::sha1hash::Sha1Hash;
use crate::utils::bencode::BencodedValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFile {
    bencoded_dict: BencodedValue,
}

impl TorrentFile {
    pub async fn new(torrent_file_name: &std::path::Path) -> Result<TorrentFile> {
        let torrent_file_bytes = read_file_as_bytes(torrent_file_name).await.context("couldn't read file as bytes")?;
        let bencoded_dict = BencodedValue::from_bytes(&torrent_file_bytes)?;

        if !bencoded_dict.torrent_file_is_valid() {
            return Err(anyhow!("[Error] Invalid dictionary: doesn't have all the required keys"));
        }

        Ok(TorrentFile { bencoded_dict })
    }

    pub fn get_bencoded_dict_ref(&self) -> &BencodedValue {
        &self.bencoded_dict
    }

    pub fn get_piece_size(&self, piece_index: usize) -> Result<usize> {
        let piece_length = self.get_piece_length()?;

        if piece_index != self.get_pieces_count()? -1 {
            return Ok(piece_length);
        }

        match self.get_torrent_length()? as usize % piece_length {
            0 => Ok(piece_length),
            size => Ok(size)
        }
    }

    pub fn get_block_length(&self) -> Result<usize> {
        Ok(crate::BLOCK_SIZE)
    }

    pub fn get_pieces_count(&self) -> Result<usize> {
        let info_dict = self.bencoded_dict.get_from_dict(b"info")?;
        let pieces = info_dict.get_from_dict(b"pieces")?;
        let pieces = pieces.try_into_byte_sha1_hashes()?;

        Ok(pieces.len())
    }

    pub fn get_blocks_in_piece(&self) -> Result<usize> {
        let piece_size = self.get_piece_size(0)?;
        let block_size = self.get_block_length()?;
        
        Ok(piece_size.div_ceil(block_size))
    }

    pub fn get_blocks_count(&self) -> Result<usize> {
        let pieces_count = self.get_pieces_count()?;

        let piece_size = self.get_piece_size(0)?;
        let block_size = self.get_block_length()?;
        let blocks_in_piece = piece_size.div_ceil(block_size);

        let last_piece_size = self.get_piece_size(pieces_count - 1)?;
        let blocks_in_last_piece = last_piece_size.div_ceil(block_size);

        Ok((pieces_count - 1) * blocks_in_piece + blocks_in_last_piece)
    }

    pub fn get_info_hash(bencoded_dict: &BencodedValue) -> Result<Sha1Hash> {
        let info_dict = bencoded_dict.get_from_dict(b"info")?;
        match info_dict {
            BencodedValue::Dict(_) => {
                let bencoded_info_dict = info_dict.as_bytes()?;
                Ok(sha1_hash(bencoded_info_dict))
            },
            _ => Err(anyhow!("Invalid dictionary in info key when getting the tracker params"))
        }
    }

    pub fn get_piece_hash(&self, piece_index: usize) -> Result<Sha1Hash> {
        let info_dict = self.bencoded_dict.get_from_dict(b"info")?;
        let pieces = info_dict.get_from_dict(b"pieces")?;
        let pieces = pieces.try_into_byte_sha1_hashes()?;

        match pieces.get(piece_index) {
            Some(piece_hash) => Ok(piece_hash.clone()),
            None => Err(anyhow!("Invalid piece index in info dict ref in torrent file: {:?}", self.bencoded_dict))
        }
    }

    pub fn get_piece_length(&self) -> Result<usize> {
        let info_dict = self.bencoded_dict.get_from_dict(b"info")?;
        let piece_length = info_dict.get_from_dict(b"piece length")?;
        let piece_length = piece_length.try_into_integer()?;

        Ok(piece_length as usize)
    }

    pub fn get_torrent_length(&self) -> Result<u64> {
        let torrent_dict = self.get_bencoded_dict_ref().try_into_dict()?;
        let info_dict = match torrent_dict.get(&b"info".to_vec()) {
            Some(info_dict) => info_dict,
            None => return Err(anyhow!("Could not get info dict from torrent file ref: {:?}", self.bencoded_dict))
        };

        let mut total_size: u64 = 0;
        if let Ok(files_dict) = info_dict.get_from_dict(b"files") {
            let files = files_dict.try_into_list()?;

            for file in files {
                let file = file.try_into_dict()?;

                let length = match file.get(&b"length".to_vec()) {
                    Some(length) => length,
                    None => {
                        return Err(anyhow!("Could not get file size from info dict ref in torrent file"));
                    }
                };
                let length = length.try_into_integer()?;

                total_size += length as u64;
            }
        }
        else { 
            let file_length = info_dict.get_from_dict(b"length")?;
            let file_length = file_length.try_into_integer()?;
            
            total_size = file_length as u64;
        }
        
        Ok(total_size)
    }
}