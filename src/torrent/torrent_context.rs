
use tokio::sync::Mutex;
use anyhow::{Result, Context};

use std::sync::Arc;

use crate::utils::sha1hash::Sha1Hash;
use crate::peer::{BlockPicker, PeerAddress};
use crate::peer::peer_message::ConnectionType;

use super::{TorrentFile, TorrentInfo, TorrentState};


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
    pub async fn from_state(torrent_state: TorrentState, info_hash: Sha1Hash, connection_type: ConnectionType) -> Result<Self> {
        let needed = BlockPicker::from_state(torrent_state.needed);

        let torrent_file_path = format!("{}/{}.torrent", unsafe { crate::CLIENT_OPTIONS.state_torrent_files_path.clone() }, torrent_state.torrent_name);
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