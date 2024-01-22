use serde::{Serialize, Deserialize};
use tokio::sync::oneshot;

use crate::{torrent::TorrentState, peer::{PieceBlock, PeerAddress}};

#[derive(Debug, Serialize, Deserialize)]
pub enum ExitCode {
    SUCCESS,
    InvalidSrcOrDst,
}

#[derive(Debug)]
pub enum ClientMessage {
    Shutdown,
    Download{src: String, dst: String},
    DownloadedBlock{block: PieceBlock},
    FinishedDownloading,
    SendTorrentInfo{tx: oneshot::Sender<TorrentState>},
    TorrentsInfo{torrents: Vec<TorrentState>},
    SendTorrentsInfo,
    TerminalClientClosed,
    PeerDisconnected{peer_address: PeerAddress},
    Cancel{index: u32, begin: u32, length: u32},
    Have{piece: u32},
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TerminalClientMessage {
    Status{exit_code: ExitCode},
    Shutdown,
    Download{src: String, dst: String},
    ListTorrents{client_id: u32},
    TorrentsInfo{torrents: Vec<TorrentState>},
    TerminalClientClosed{client_id: u32},
}