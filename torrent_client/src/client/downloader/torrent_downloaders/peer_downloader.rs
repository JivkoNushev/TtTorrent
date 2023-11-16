use std::sync::Arc;

use tokio::{sync::{mpsc, Mutex}, fs::File, net::TcpStream};

use crate::torrent::{Torrent, Sha1Hash};
use crate::peer::Peer;
use crate::peer::peer_messages::PeerMessage;

pub struct PeerDownloader {
    pub peer_id: [u8;20],
    pub handler_rx: mpsc::Receiver<String>
}

impl PeerDownloader {
    pub async fn new(peer_id: [u8;20], handler_rx: mpsc::Receiver<String>) -> PeerDownloader {
        PeerDownloader {
            peer_id,
            handler_rx
        }
    } 
}

pub struct PeerDownloaderHandler {
    peer: Peer,
    torrent: Torrent,
    file: Arc<Mutex<File>>,
    downloader_tx: mpsc::Sender<String>,
}

impl PeerDownloaderHandler {

    pub async fn new(peer: Peer, torrent: Torrent, file: Arc<Mutex<File>>,downloader_tx: mpsc::Sender<String>) -> PeerDownloaderHandler {
        PeerDownloaderHandler {
            peer,
            torrent,
            file,
            downloader_tx
        }
    }

    async fn check_hash(l: &[u8;20], r: [u8;20]) -> bool {
        l as &[u8] == r
    }

    async fn handshake(&mut self, stream: &mut TcpStream) {
        // send handshake
        PeerMessage::send_handshake(stream, &self.torrent.info_hash, &self.peer.id).await;
        
        // receive handshake response
        let handshake_response = PeerMessage::recv_handshake(stream).await;
        println!("Handshake received: {:?}", handshake_response);

        // check info hash
        if !PeerDownloaderHandler::check_hash(self.torrent.get_info_hash_ref().await.get_hash_ref(), handshake_response.info_hash).await {
            panic!("Info hash doesn't match handshake info hash");
        }

        // save peer id
        self.peer.id = handshake_response.peer_id;
    } 

    async fn interested(&mut self, stream: &mut TcpStream) {
        // send interested
        PeerMessage::send_interested(stream).await;

        // receive interested
        let interested_response = PeerMessage::recv_interested(stream).await;
        println!("Interested received: {:?}", interested_response);

    }

    async fn unchoke(&mut self, stream: &mut TcpStream) {
        // send interested
        PeerMessage::send_unchoke(stream).await;

        // receive interested
        let unchoke_response = PeerMessage::recv_unchoke(stream).await;
        println!("Interested received: {:?}", unchoke_response);

    }

    pub async fn run(mut self) {
        // let msg = format!("Hello from PeerDownloaderHandler: {:?}", self.peer.id);

        // let _ = self.downloader_tx.send(msg).await.map_err(|e| {
        //     println!("Error sending from PeerDownloader: {}", e);
        // });

        // println!("Torrent info hash: {:?}", self.torrent.info_hash);

        // println!("Peer id: {:?}", self.peer.id);

        // establish connection
        // TODO: make a loop for the connection
        let mut stream = TcpStream::connect(format!("{}:{}", self.peer.address, self.peer.port)).await;

        let mut counter = 0;
        while let Err(_) = stream {
            stream = TcpStream::connect(format!("{}:{}", self.peer.address, self.peer.port)).await;

            // println!("Trying {} for {} time", self.peer.address, counter);

            if counter >= 100 {
                break;
            }

            counter += 1;
        }

        let mut stream = match stream {
            Ok(stream) => {
                println!("Connected to peer: {:?}", stream);
                stream
            },
            Err(e) => {
                // eprintln!("Couldn't connect to peer: {}", e);
                return;
            }
        };

        self.handshake(&mut stream).await;

        // wait for bitmap

        // send interested

        // receive unchoke
    }
    
}