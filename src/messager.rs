use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, oneshot};

use crate::peer::{Block, PeerAddress, PeerSession};
use crate::torrent::torrent_state::TorrentState;
use crate::utils::ExitCode;


#[derive(Debug)]
pub enum ClientMessage {
    Shutdown,
    AddTorrent{src: String, dst: String},
    DownloadedBlock{block: Block},
    FinishedDownloading,
    SendTorrentInfo{tx: oneshot::Sender<TorrentState>},
    TorrentsInfo{torrents: Vec<TorrentState>},
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
    TorrentsInfo{torrents: Vec<TorrentState>},
    TerminalClientClosed,
}