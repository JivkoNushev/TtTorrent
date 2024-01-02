use serde::{Serialize, Deserialize};
use tokio::sync::oneshot;

use crate::torrent::TorrentState;

#[derive(Debug)]
pub enum ClientMessage {
    Shutdown,
    Download{src: String, dst: String},
    DownloadedPiece{piece_index: usize, piece: Vec<u8>},
    FinishedDownloading,
    SendTorrentInfo{tx: oneshot::Sender<TorrentState>},
    TorrentsInfo{torrents: Vec<TorrentState>},
    SendTorrentsInfo,
    TerminalClientClosed,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TerminalClientMessage {
    Shutdown,
    Download{src: String, dst: String},
    ListTorrents,
    TorrentsInfo{torrents: Vec<TorrentState>},
    TerminalClientClosed,
}