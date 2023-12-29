use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use anyhow::{Result, Context, Ok};
use serde::{Serialize, Deserialize, Serializer};

use std::sync::Arc;

use crate::messager::ClientMessage;
use crate::peer::{PeerHandle, PeerAddress};
use crate::tracker::Tracker;
use crate::peer::peer_message::ConnectionType;
use crate::disk_writer::DiskWriterHandle;

pub mod torrent_file;
pub use torrent_file::{TorrentFile, Sha1Hash, BencodedValue};

pub mod torrent_parser;
pub use torrent_parser::TorrentParser;

#[derive(Debug, Serialize, Deserialize)]
struct TorrentState {
    pub src_path: String,
    pub dest_path: String,
    pub torrent_name: String,
    pub torrent_file: TorrentFile,
    pub info_hash: Sha1Hash,
    pub pieces: Vec<usize>,
}

impl TorrentState {
    pub async fn new(torrent_context: TorrentContext) -> Self {
        Self {
            src_path: torrent_context.src_path,
            dest_path: torrent_context.dest_path,
            torrent_name: torrent_context.torrent_name,
            torrent_file: torrent_context.torrent_file,
            info_hash: torrent_context.info_hash,
            pieces: torrent_context.pieces.lock().await.clone(),
        }
    }
}

#[derive(Debug)]
struct TorrentContext {
    pub connection_type: ConnectionType,

    pub src_path: String,
    pub dest_path: String,
    pub torrent_name: String,
    pub torrent_file: TorrentFile,
    pub info_hash: Sha1Hash,
    pub pieces: Arc<Mutex<Vec<usize>>>,
}

struct Torrent {
    tx: mpsc::Sender<ClientMessage>,
    rx: mpsc::Receiver<ClientMessage>,
    peer_handles: Vec<PeerHandle>,
    disk_writer_handle: DiskWriterHandle,
    
    pub torrent_context: TorrentContext,

    pub client_id: [u8; 20],
}

// static functions for Torrent
impl Torrent {
    async fn save_state(torrent_context: TorrentContext) -> Result<()> {
        let connection_type = torrent_context.connection_type.clone();
        let torrent_context = TorrentState::new(torrent_context).await;

        let client_state = match tokio::fs::read_to_string("./client_state.state").await {
            std::result::Result::Ok(client_state) => client_state,
            Err(_) => String::new(),
        };

        let mut client_state: serde_json::Value = match client_state.len() {
            0 => serde_json::json!({}),
            _ => serde_json::from_str(&client_state).unwrap(),
        };

        let torrent_name = torrent_context.torrent_name.clone();
        let connection_type = match connection_type {
            ConnectionType::Incoming => "Seeding",
            ConnectionType::Outgoing => "Downloading",
        };

        let torrent_context = serde_json::to_value(torrent_context).unwrap();

        client_state[connection_type][torrent_name] = torrent_context;

        let client_state = serde_json::to_string_pretty(&client_state).unwrap();

        tokio::fs::write("./client_state.state", client_state).await.context("couldn't write to client state file")?;

        Ok(())
    }
}

impl Torrent {
    pub async fn new(client_id: [u8; 20], sender: mpsc::Sender<ClientMessage>, receiver: mpsc::Receiver<ClientMessage>, src: &str, dest: &str) -> Result<Self> {
        let torrent_file = TorrentFile::new(&src).await.context("couldn't create TorrentFile")?;

        let torrent_name = std::path::Path::new(src)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        let info_hash = match TorrentFile::get_info_hash(torrent_file.get_bencoded_dict_ref()) {
            Some(info_hash) => info_hash,
            None => panic!("Could not get info hash from torrent file: {}", torrent_name)
        };

        // not downloaded piece indexes
        let pieces_left = {
            let torrent_dict = match torrent_file.get_bencoded_dict_ref().try_into_dict() {
                Some(torrent_dict) => torrent_dict,
                None => panic!("Could not get torrent dict ref from torrent file: {}", torrent_name)
            };
            let info_dict = match torrent_dict.get("info") {
                Some(info_dict) => info_dict,
                None => panic!("Could not get info dict from torrent file ref: {}", torrent_name)
            };
            let pieces = match info_dict.get_from_dict("pieces") {
                Some(pieces) => pieces,
                None => panic!("Could not get pieces from info dict ref in torrent file: {}", torrent_name)
            };

            let pieces = match pieces {
                BencodedValue::ByteSha1Hashes(pieces) => pieces,
                _ => panic!("Could not get pieces from info dict ref in torrent file: {}", torrent_name)
            };

            let pieces_left = (0..pieces.len()).collect::<Vec<usize>>();

            pieces_left
        };

        let disk_writer_handle = DiskWriterHandle::new(dest.to_string(), torrent_name.to_string(), torrent_file.clone());
        
        let torrent_context = TorrentContext {
            connection_type: ConnectionType::Outgoing, // TODO: make this configurable

            src_path: src.to_string(),
            dest_path: dest.to_string(),
            torrent_name: torrent_name.to_string(),
            torrent_file,
            info_hash,
            pieces: Arc::new(Mutex::new(pieces_left)),
        };

        Ok(Self {
            tx: sender,
            rx: receiver,
            peer_handles: Vec::new(),
            disk_writer_handle,

            torrent_context,
            client_id,
        })
    }

    fn add_new_peers(&mut self, mut peer_addresses: Vec<PeerAddress>, connection_type: ConnectionType) {
        let old_peer_addresses = self.peer_handles.iter().map(|peer_handle| peer_handle.peer_address.clone()).collect::<Vec<PeerAddress>>();
        
        peer_addresses.retain(|peer_address| !old_peer_addresses.contains(peer_address));

        let piece_length = self.torrent_context.torrent_file.get_piece_length();
        let torrent_length = self.torrent_context.torrent_file.get_torrent_length();

        for peer_address in peer_addresses {
            let peer_handle = PeerHandle::new(self.client_id, connection_type.clone(), self.tx.clone(), piece_length, torrent_length,
                 self.torrent_context.info_hash.clone(), Arc::clone(&self.torrent_context.pieces), peer_address);
            self.peer_handles.push(peer_handle);
        }
    }

    async fn download(&mut self, tracker_response: reqwest::Response) {
        let peer_addresses;
        
        if crate::DEBUG_MODE {
            // peer_addresses = vec![PeerAddress{address: "127.0.0.1".to_string(), port: "37051".to_string()}];
            peer_addresses = vec![PeerAddress{address: "192.168.0.15".to_string(), port: "51413".to_string()}];
        }
        else {
            // get peers from tracker response
            peer_addresses = PeerHandle::peers_from_tracker_response(tracker_response).await;
        }

        // add new peers
        self.add_new_peers(peer_addresses, self.torrent_context.connection_type.clone());
    }

    pub async fn run(mut self) -> Result<()> {
        let tracker_url = match Tracker::tracker_url_get(self.torrent_context.torrent_file.get_bencoded_dict_ref()) {
            Some(tracker_url) => tracker_url,
            None => panic!("Could not get tracker url from torrent file: {}", self.torrent_context.torrent_name)
        };

        let mut tracker = Tracker::new(&tracker_url).await;
        
        // run tracker with default params
        let tracker_response = tracker.default_params(&self.torrent_context.info_hash, self.client_id).await.context("couldn't get response with default parameters")?;
        self.download(tracker_response).await;

        // TODO: exit when torrent is finished
        loop {
            tokio::select! {
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Shutdown => {
                            // send stop message to all peers
                            for peer_handle in &mut self.peer_handles {
                                let _ = peer_handle.tx.send(ClientMessage::Shutdown).await;
                            }

                            // send stop message to disk writer
                            self.disk_writer_handle.shutdown().await?;

                            break;
                        }
                        ClientMessage::DownloadedPiece { piece_index, piece } => {
                            self.disk_writer_handle.downloaded_piece(piece_index, piece).await?;
                        },
                        _ => {}
                    }
                },
                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                    // TODO: get more peers
                    continue;
                    let tracker_response = todo!("Tracker response with updated params based on current state");
                    let rxs = self.download(tracker_response).await;
                }
            }
        }

        for peer_handle in self.peer_handles {
            let _ = peer_handle.join_handle.await;
        }

        self.disk_writer_handle.join_handle.await?;

        Torrent::save_state(self.torrent_context).await?;

        Ok(())
    }
    
}

pub struct TorrentHandle {
    pub tx: mpsc::Sender<ClientMessage>,
    pub join_handle: JoinHandle<()>,
}

impl TorrentHandle {
    pub async fn new(client_id: [u8; 20], src: &str, dest: &str) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(100);
        let torrent = Torrent::new(client_id, sender.clone(), receiver, src, dest).await?;

        let join_handle = tokio::spawn(async move {
            if let Err(e) = torrent.run().await {
                println!("Torrent error: {:?}", e);
            }
        });

        Ok(Self {
            tx: sender,
            join_handle,
        })
    }
}