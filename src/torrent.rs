use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

use anyhow::{Result, Context};

use std::sync::Arc;

use crate::messager::ClientMessage;
use crate::peer::{PeerHandle, PeerAddress, peer_address};
use crate::tracker::Tracker;
use crate::peer::peer_message::ConnectionType;
use crate::disk_writer::DiskWriterHandle;

pub mod torrent_file;
pub use torrent_file::{TorrentFile, Sha1Hash, BencodedValue};

pub mod torrent_parser;
pub use torrent_parser::TorrentParser;

struct Torrent {
    tx: mpsc::Sender<ClientMessage>,
    rx: mpsc::Receiver<ClientMessage>,
    peer_handles: Vec<PeerHandle>,
    disk_writer_handle: DiskWriterHandle,
    
    pub src_path: String,
    pub dest_path: String,
    pub torrent_name: String,
    pub torrent_file: TorrentFile,
    pub info_hash: Sha1Hash,
    pub pieces: Arc<Mutex<Vec<usize>>>,

    pub client_id: [u8; 20],
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
        
        Ok(Self {
            tx: sender,
            rx: receiver,
            peer_handles: Vec::new(),
            disk_writer_handle,

            src_path: src.to_string(),
            dest_path: dest.to_string(),
            torrent_name: torrent_name.to_string(),
            torrent_file,
            info_hash,
            pieces: Arc::new(Mutex::new(pieces_left)),
            client_id,
        })
    }

    fn add_new_peers(&mut self, mut peer_addresses: Vec<PeerAddress>) {
        let old_peer_addresses = self.peer_handles.iter().map(|peer_handle| peer_handle.peer_address.clone()).collect::<Vec<PeerAddress>>();
        
        peer_addresses.retain(|peer_address| !old_peer_addresses.contains(peer_address));

        let piece_length = self.torrent_file.get_piece_length();
        let torrent_length = self.torrent_file.get_torrent_length();

        for peer_address in peer_addresses {
            let peer_handle = PeerHandle::new(self.client_id, ConnectionType::Outgoing, self.tx.clone(), piece_length, torrent_length,
                 self.info_hash.clone(), Arc::clone(&self.pieces), peer_address);
            self.peer_handles.push(peer_handle);
        }
    }

    pub async fn download(&mut self, tracker_response: reqwest::Response) {
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
        self.add_new_peers(peer_addresses)
    }

    pub async fn run(mut self) -> Result<()> {
        let tracker_url = match Tracker::tracker_url_get(self.torrent_file.get_bencoded_dict_ref()) {
            Some(tracker_url) => tracker_url,
            None => panic!("Could not get tracker url from torrent file: {}", self.torrent_name)
        };

        let mut tracker = Tracker::new(&tracker_url).await;
        
        // run tracker with default params
        let tracker_response = tracker.default_params(&self.info_hash, self.client_id).await.context("couldn't get response with default parameters")?;
        let mut peer_piece_rxs = self.download(tracker_response).await;

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
                            // todo!("send piece to disk_handle")
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