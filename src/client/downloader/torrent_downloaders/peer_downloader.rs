use std::{sync::Arc, fmt::Result};

use tokio::{sync::{mpsc, Mutex}, fs::File, net::TcpStream, io::{AsyncWriteExt, AsyncReadExt}};

use crate::{torrent::{Torrent, Sha1Hash}, peer::peer_messages::{Message, MessageID, Handshake}, utils::AsBytes};
use crate::peer::Peer;
use crate::peer::peer_messages::PeerMessage;

pub struct PeerDownloader {
    pub peer_id: [u8;20],
    pub handler_rx: mpsc::Receiver<String>
}

impl PeerDownloader {
    pub fn new(peer_id: [u8;20], handler_rx: mpsc::Receiver<String>) -> PeerDownloader {
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

    pub fn new(peer: Peer, torrent: Torrent, file: Arc<Mutex<File>>,downloader_tx: mpsc::Sender<String>) -> PeerDownloaderHandler {
        PeerDownloaderHandler {
            peer,
            torrent,
            file,
            downloader_tx
        }
    }

    fn check_hash(l: &[u8;20], r: [u8;20]) -> bool {
        l as &[u8] == r
    }

    fn response_is_ok(response: &Vec<u8>) -> bool {
        response == &[1]
    }

    async fn interested(&mut self, stream: &mut TcpStream) {
        // send interested
        PeerMessage::send_interested(stream).await;

        // receive interested
        let interested_response = PeerMessage::recv_interested(stream).await;
        println!("Interested received from peer {}: {:?}", self.peer.address, interested_response);

        if PeerDownloaderHandler::response_is_ok(&interested_response) {
            self.peer.am_interested = true;
        }
        else {
            self.peer.am_interested = false;
        }
    }

    async fn unchoke(&mut self, stream: &mut TcpStream) {
        // send interested
        PeerMessage::send_unchoke(stream).await;

        // receive interested
        let unchoke_response = PeerMessage::recv_unchoke(stream).await;
        println!("Unchoke received from peer {}: {:?}", self.peer.address, unchoke_response);

        if PeerDownloaderHandler::response_is_ok(&unchoke_response) {
            self.peer.choking = false;
        }
        else {
            self.peer.am_interested = true;
        }
    }

    async fn bitfield(&mut self, stream: &mut TcpStream) {
        let bitfield_response = PeerMessage::recv_bitfield(stream).await;
        println!("Received bitfield from peer '{}': {:?}", self.peer.address, bitfield_response);        
    }

    async fn request_piece(&mut self, stream: &mut TcpStream, piece_index: u32) -> Vec<u8> {
        let piece_length = self.torrent.get_piece_length();

        const BLOCK_SIZE: u32 = 1 << 14;

        let block_count: u32 = piece_length as u32 / BLOCK_SIZE;
        let last_block_size: u32 = piece_length as u32 % BLOCK_SIZE;

        let mut piece: Vec<u8> = vec![0; piece_length as usize];

        for i in 0..block_count {
            PeerMessage::send_request(stream, piece_index, BLOCK_SIZE * i, BLOCK_SIZE).await;

            let piece_response_block = PeerMessage::recv_piece(stream).await;
            println!("Piece block response from peer {}: {:?}", self.peer.address, piece_response_block);

            // add to piece 
            let piece_response_block = &piece_response_block[9..];
            piece.append(&mut piece_response_block.to_vec());
        }
        // last piece
        PeerMessage::send_request(stream, piece_index, block_count, last_block_size).await;

        let piece_response_block = PeerMessage::recv_piece(stream).await;
        println!("Piece block response from peer {}: {:?}", self.peer.address, piece_response_block);

        // add to piece 
        let piece_response_block = &piece_response_block[9..];
        piece.append(&mut piece_response_block.to_vec());

        println!("The whole piece {piece_index}: {piece:?}");

        piece
    }

    async fn send(&mut self, stream: &mut TcpStream, message: Message) {
        match stream.write_all(&message.as_bytes()).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't send message {}", e)
        }
    }

    async fn recv(&mut self, stream: &mut TcpStream, message: &mut Message) {
        let mut recv_size: [u8; 4] = [0; 4];
        match stream.read_exact(&mut recv_size).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive message {}", e)
        }

        let recv_size = u32::from_be_bytes(recv_size);

        let mut buf: Vec<u8> = vec![0; recv_size as usize];

        match stream.read_exact(&mut buf).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't receive message {}", e)
        }

        message.id = MessageID::from(buf[0]);
        message.payload = buf[1..].to_vec();
    }

    async fn send_handshake(&mut self, stream: &mut TcpStream) {
        let handshake = Handshake::new( self.torrent.info_hash.as_bytes().clone(), self.peer.id.clone());

        match stream.write_all(&handshake.as_bytes()).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't send handshake {}", e)
        }

        println!("Handshake sent to peer: {:?}", self.peer.address);
    }

    async fn recv_handshake(&mut self, stream: &mut TcpStream) -> tokio::io::Result<[u8;20]> {
        let mut buf = vec![0; 68];

        stream.read_exact(&mut buf).await.map_err(|e| {
            tokio::io::Error::new(tokio::io::ErrorKind::Other, format!("Error: couldn't receive handshake response from peer {}: {}", self.peer.address, e))
        })?;

        if buf.len() != 68 {
            return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Invalid handshake length"));
        }

        let handshake = Handshake::from_bytes(buf);

        // check info hash
        if !PeerDownloaderHandler::check_hash(&self.torrent.get_info_hash_ref().as_bytes(), handshake.info_hash) {
            return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Info hash doesn't match handshake info hash"));
        }

        // save peer id
        self.peer.id = handshake.peer_id;

        println!("Handshake received from peer: {:?}", self.peer.address);

        Ok(handshake.peer_id)
    }

    pub async fn run(mut self) {
        // establish connection
        let mut stream = TcpStream::connect(format!("{}:{}", self.peer.address, self.peer.port)).await;

        let mut counter = 0;
        while let Err(_) = stream {
            stream = TcpStream::connect(format!("{}:{}", self.peer.address, self.peer.port)).await;

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
                eprintln!("Couldn't connect to peer: {}", e);
                return;
            }
        };

        // self.handshake(&mut stream).await;
        // self.bitfield(&mut stream).await;
        // self.interested(&mut stream).await;
        // self.unchoke(&mut stream).await;

        // send a handshake
        self.send_handshake(&mut stream).await;

        match self.recv_handshake(&mut stream).await {
            Ok(_) => {},
            Err(e) => {
                eprintln!("Error: couldn't receive handshake from peer: {}", e);
                return;
            }
        }


        // if self.peer.choking || !self.peer.am_interested {
        //     // TODO: maybe rerun function ?

        //     println!("Peer {{ choking: {}, am_interested: {} }}", self.peer.choking, self.peer.am_interested);
        //     return;
        // }
        // else {
        //     println!("Peer ready for requests");
        // }

        // test by downloading the first piece
        // let piece = self.request_piece(&mut stream, 0).await;

        // save the piece
        // println!("Saving piece 0: {piece:?}");

    }
    
}