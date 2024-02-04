use tokio::sync::{mpsc, Mutex};

use std::sync::Arc;

use crate::torrent::TorrentInfo;
use crate::messager::ClientMessage;
use crate::utils::Sha1Hash;

use super::{BlockPicker, PeerAddress};


pub struct PeerTorrentContext {
    pub tx: mpsc::Sender<ClientMessage>,

    pub torrent_info: Arc<TorrentInfo>,
    pub info_hash: Sha1Hash,
    pub needed: Arc<Mutex<BlockPicker>>,
    pub bitfield: Arc<Mutex<Vec<u8>>>,

    pub uploaded: Arc<Mutex<u64>>
}

impl PeerTorrentContext {
    pub fn new(tx: mpsc::Sender<ClientMessage>, torrent_info: Arc<TorrentInfo>, info_hash: Sha1Hash, needed: Arc<Mutex<BlockPicker>>, bitfield: Arc<Mutex<Vec<u8>>>, uploaded: Arc<Mutex<u64>>) -> Self {
        Self {
            tx,

            torrent_info,
            info_hash,
            needed,
            bitfield,
            uploaded,
        }
    }
}

pub(super) struct PeerContext {
    pub id: [u8;20],
    pub ip: PeerAddress,
    pub am_interested: bool,
    pub am_choking: bool,
    pub interested: bool,
    pub choking: bool,
    pub bitfield: Vec<u8>,
}