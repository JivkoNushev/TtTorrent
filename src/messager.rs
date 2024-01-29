use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, oneshot};

use crate::{torrent::TorrentContextState, peer::{Block, PeerAddress, PeerSession}};

#[derive(Debug, Serialize, Deserialize)]
pub enum ExitCode {
    SUCCESS,
    InvalidSrcOrDst,
}

#[derive(Debug)]
pub enum ClientMessage {
    Shutdown,
    AddTorrent{src: String, dst: String},
    DownloadedBlock{block: Block},
    FinishedDownloading,
    SendTorrentInfo{tx: oneshot::Sender<TorrentContextState>},
    TorrentsInfo{torrents: Vec<TorrentContextState>},
    SendTorrentsInfo,
    AddPeerSession{peer_session: PeerSession},
    TerminalClientClosed,
    PeerDisconnected{peer_address: PeerAddress},
    Request{block: Block, tx: mpsc::Sender<ClientMessage>},
    RequestedBlock{block: Block},
    Cancel{block: Block},
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