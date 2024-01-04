use serde::{Serialize, Deserialize};
use tokio::sync::oneshot;

use crate::{torrent::TorrentState, peer::PeerAddress};

#[derive(Debug, Serialize, Deserialize)]
pub enum ExitCode {
    SUCCESS,
    InvalidSrcOrDst,
}

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
    PeerDisconnected{peer_address: PeerAddress},
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