use serde::{Serialize, Deserialize};
use tokio::sync::Mutex;
use anyhow::{Context, Result};

use std::sync::Arc;

use crate::peer::{PeerAddress, BlockPicker, BlockPickerState, ConnectionType};
use crate::utils::sha1hash::Sha1Hash;

use super::TorrentInfo;
pub use super::torrent_file::TorrentFile;

#[derive(Debug, Serialize, Deserialize)]
pub struct TorrentContextState {
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

impl TorrentContextState {
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

#[derive(Debug, Clone)]
pub struct TorrentContext {
    pub connection_type: ConnectionType,

    pub src_path: String,
    pub dest_path: String,
    pub torrent_name: String,
    pub torrent_file: Arc<TorrentFile>,
    pub info_hash: Sha1Hash,
    pub needed: Arc<Mutex<BlockPicker>>,
    pub bitfield: Arc<Mutex<Vec<u8>>>,
    pub peers: Vec<PeerAddress>,

    pub torrent_info: Arc<TorrentInfo>,

    pub downloaded: Arc<Mutex<u64>>,
    pub uploaded: Arc<Mutex<u64>>,
}

impl TorrentContext {
    pub async fn from_state(torrent_state: TorrentContextState, info_hash: Sha1Hash, connection_type: ConnectionType) -> Result<Self> {
        let needed = BlockPicker::from_state(torrent_state.needed);

        let torrent_file_path = format!("{}/{}.torrent", crate::STATE_TORRENT_FILES_PATH, torrent_state.torrent_name);
        let path = std::path::Path::new(&torrent_file_path);

        let torrent_file = TorrentFile::new(path).await.context("couldn't create TorrentFile")?;

        Ok(Self {
            connection_type,

            src_path: torrent_state.src_path,
            dest_path: torrent_state.dest_path,
            torrent_name: torrent_state.torrent_name,
            torrent_file: Arc::new(torrent_file),
            info_hash: info_hash,
            needed: Arc::new(Mutex::new(needed)),
            bitfield: Arc::new(Mutex::new(torrent_state.bitfield)),
            peers: torrent_state.peers,

            torrent_info: Arc::new(torrent_state.torrent_info),

            downloaded: Arc::new(Mutex::new(torrent_state.downloaded)),
            uploaded: Arc::new(Mutex::new(torrent_state.uploaded)),
        })
    }
}