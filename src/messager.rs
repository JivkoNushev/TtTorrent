use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, oneshot};

use crate::{torrent::TorrentContextState, peer::{PeerAddress, PeerSession, PieceBlock}};

#[derive(Debug, Serialize, Deserialize)]
pub enum ExitCode {
    SUCCESS,
    InvalidSrcOrDst,
}

#[derive(Debug)]
pub enum ClientMessage {
    Shutdown,
    AddTorrent{src: String, dst: String},
    DownloadedBlock{piece_block: PieceBlock},
    FinishedDownloading,
    SendTorrentInfo{tx: oneshot::Sender<TorrentContextState>},
    TorrentsInfo{torrents: Vec<TorrentContextState>},
    SendTorrentsInfo,
    AddPeerSession{peer_session: PeerSession},
    TerminalClientClosed,
    PeerDisconnected{peer_address: PeerAddress},
    Request{piece_block: PieceBlock, tx: mpsc::Sender<ClientMessage>},
    RequestedBlock{piece_block: PieceBlock},
    Cancel{index: u32, begin: u32, length: u32},
    Have{piece: u32},
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TerminalClientMessage {
    Status{exit_code: ExitCode},
    Shutdown,
    AddTorrent{src: String, dst: String},
    ListTorrents,
    TorrentsInfo{torrents: Vec<TorrentContextState>},
    TerminalClientClosed,
}