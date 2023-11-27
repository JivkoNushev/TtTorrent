use std::{sync::Arc, fmt::Result, f32::consts::E, mem, collections::BTreeMap, ops::Index};

use tokio::{sync::{mpsc, Mutex}, fs::File, net::TcpStream, io::{AsyncWriteExt, AsyncReadExt, AsyncSeekExt}};
use rand::Rng;

use crate::{torrent::{Torrent, Sha1Hash, BencodedValue}, peer::peer_messages::{Message, MessageID, Handshake}, utils::AsBytes, client::{CLIENT_PEER_ID, Client}};
use crate::peer::Peer;
use crate::utils::sha1_hash;


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
    torrent: Arc<Mutex<Torrent>>,
    file: Arc<Mutex<File>>,
    downloader_tx: mpsc::Sender<String>,
}

impl PeerDownloaderHandler {

    pub fn new(peer: Peer, torrent: Arc<Mutex<Torrent>>, file: Arc<Mutex<File>>,downloader_tx: mpsc::Sender<String>) -> PeerDownloaderHandler {
        PeerDownloaderHandler {
            peer,
            torrent,
            file,
            downloader_tx
        }
    }

    async fn get_info_hash(&self) -> [u8; 20] {
        let torrent_guard = self.torrent.lock().await;

        torrent_guard.info_hash.as_bytes().clone()
    }

    async fn get_piece_length(&self) -> u32 {
        let torrent_guard = self.torrent.lock().await;

        torrent_guard.get_piece_length() as u32
    }

    async fn get_piece_count(&self) -> usize {
        let torrent_guard = self.torrent.lock().await;

        torrent_guard.get_piece_count().try_into().unwrap()
    }

    async fn get_file_size(&self) -> u32 {
        let torrent_guard = self.torrent.lock().await;

        torrent_guard.get_file_size()
    }

    async fn get_piece_hash(&self, piece_index: usize) -> Option<[u8; 20]> {
        let torrent_guard = self.torrent.lock().await;

        if let Some(piece) = torrent_guard.torrent_file.get_piece_hash(piece_index) {
            Some(piece.as_bytes().clone())
        }
        else {
            None
        }
    }

    async fn get_files(&self) -> Vec<BTreeMap<String, BencodedValue>> {
        let torrent_guard = self.torrent.lock().await;

        torrent_guard.get_files()
    }

    async fn get_random_not_downloaded_piece(&self) -> usize {
        let mut torrent_guard = self.torrent.lock().await;

        let mut rng = rand::thread_rng();

        let piece_index = rng.gen_range(0..torrent_guard.pieces_left.len());

        let piece = torrent_guard.pieces_left[piece_index] as usize;

        torrent_guard.pieces_left.remove(piece_index);

        piece
    }

    async fn pieces_left(&self) -> bool {
        let torrent_guard = self.torrent.lock().await;

        !torrent_guard.pieces_left.is_empty()
    }

    async fn write_to_file(&self, piece: Vec<u8>, piece_index: usize, files: &Vec<BTreeMap<String, BencodedValue>>) {
        // get files by piece index
        // write to files
        
        let piece_length = self.get_piece_length().await;

        let piece_offset = piece_index as u32 * piece_length;

        let mut length_sum = 0;

        let mut files_index = 0;
        let mut file_offset = 0;

        let mut pieces_offset = 0;
        if piece_offset != 0 {
            for (i, file) in files.iter().enumerate() {
                if let BencodedValue::Integer(file_length) = file.get("length").unwrap() {
                    length_sum += *file_length as u32;

                    if length_sum >= piece_offset {
                        files_index = i;
                        file_offset = *file_length as u32 - (length_sum - piece_offset);
                        break;
                    }
                }
            }
        }

        let mut bytes_left = piece_length;

        while bytes_left > 0 {
            let file = &files[files_index];

            let file_length = if let BencodedValue::Integer(file_length) = file.get("length").unwrap() {
                *file_length as u32
            } else {
                panic!("Error: couldn't get file length");
            };

            let file_path: Vec<String> = if let BencodedValue::List(file_path) = file.get("path").unwrap() {
                file_path.iter().map(|path| {
                    if let BencodedValue::String(path) = path {
                        path.clone()
                    } else {
                        panic!("Error: couldn't get file path");
                    }
                }).collect()
            } else {
                panic!("Error: couldn't get file path");
            };
            let file_path = file_path.join("/");

            // open if exists, create if doesnt

            // let path_buf = std::fs::(format!("test_data/folder_with_files/{file_path}")).await.unwrap();
            let path_buf = std::path::PathBuf::from(format!("test_data/folder_with_files/{file_path}"));

            let dir = path_buf.parent().unwrap();

            tokio::fs::create_dir_all(dir).await.unwrap();

            let file = tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .append(true)
                .create(true)
                .open(path_buf)
                .await
                .unwrap();

            let bytes_to_write = if bytes_left > file_length - file_offset {
                file_length - file_offset
            } else {
                bytes_left
            };

            let mut file_guard = file;

            file_guard.seek(std::io::SeekFrom::Start(file_offset as u64)).await.unwrap();

            file_guard.write_all(
                &piece[pieces_offset as usize..(pieces_offset + bytes_to_write) as usize]
            ).await.unwrap();

            bytes_left -= bytes_to_write;
        }

        // let mut file_guard = self.file.lock().await;

        // let piece_length = self.get_piece_length().await;

        // let offset = piece_index as u32 * piece_length;

        // file_guard.seek(std::io::SeekFrom::Start(offset as u64)).await.unwrap();

        // file_guard.write_all(&piece).await.unwrap();
    }

    fn check_hash(l: &[u8;20], r: &[u8;20]) -> bool {
        l == r
    }

    async fn send(&mut self, stream: &mut TcpStream, message: Message) -> std::io::Result<()> {
        stream.write_all(&message.as_bytes()).await?;
        Ok(())
    }

    async fn recv(&mut self, stream: &mut TcpStream, message_id: MessageID) -> std::io::Result<Message> {
        let mut recv_size: [u8; 4] = [0; 4];

        let mut recv_size_u32: u32;
        loop {
            stream.read_exact(&mut recv_size).await?;

            recv_size_u32 = u32::from_be_bytes(recv_size);

            println!("Received size: {:?}", recv_size_u32);

            if 0 != recv_size_u32 {
                break;
            }
        }
        
        let mut buf: Vec<u8> = vec![0; recv_size_u32 as usize];

        stream.read_exact(&mut buf).await?;

        // println!("Received message: {:?}", buf);

       

        Ok(Message::new(
            MessageID::from(buf[0]),
            buf[1..].to_vec()
        ))
    }

    async fn recv_response(&mut self, stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
        let mut recv_size: [u8; 4] = [0; 4];

        stream.read_exact(&mut recv_size).await?;

        let recv_size = u32::from_be_bytes(recv_size);

        let mut buf: Vec<u8> = vec![0; recv_size as usize];

        stream.read_exact(&mut buf).await?;

        Ok(buf)
    }

    async fn send_handshake(&mut self, stream: &mut TcpStream) {
        let handshake = Handshake::new( self.get_info_hash().await, Client::get_client_id().await);
        // println!("handshake as bytes {:?}", handshake.as_bytes());

        match stream.write_all(&handshake.as_bytes()).await {
            Ok(_) => {},
            Err(e) => panic!("Error: couldn't send handshake {}", e)
        }

        println!("Handshake sent to peer: {}", self.peer);
    }

    async fn recv_handshake(&mut self, stream: &mut TcpStream) -> tokio::io::Result<[u8;20]> {
        let mut buf = vec![0; 68];

        stream.read_exact(&mut buf).await.map_err(|e| {
            tokio::io::Error::new(tokio::io::ErrorKind::Other, format!("Error: couldn't receive handshake response from peer {}: {}", self.peer, e))
        })?;

        println!("Received handshake response from peer {}: {:?}", self.peer, buf);

        if buf.len() != 68 {
            return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Invalid handshake length"));
        }

        let handshake = Handshake::from_bytes(buf);

        // check info hash
        if !PeerDownloaderHandler::check_hash(&self.get_info_hash().await, &handshake.info_hash) {
            return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Info hash doesn't match handshake info hash"));
        }

        // save peer id
        self.peer.id = handshake.peer_id;

        println!("Handshake received from peer: {}", self.peer);

        Ok(handshake.peer_id)
    }

    async fn request_piece(&mut self, stream: &mut TcpStream, piece_index: usize) -> std::io::Result<Vec<u8>> {
        println!("Requesting piece {} from peer {}", piece_index, self.peer);
        
        let mut piece_length: u32 = self.get_piece_length().await as u32;
        if piece_index == self.get_piece_count().await - 1 {
            let file_size = self.get_file_size().await;
            piece_length = file_size % piece_length;
        }

        println!("Piece length: {piece_length}");

        const BLOCK_SIZE: u32 = 1 << 14;

        println!("Block size: {BLOCK_SIZE}");

        let block_count: u32 = piece_length / BLOCK_SIZE;

        println!("Block count: {block_count}");

        let mut piece: Vec<u8> = Vec::new();
        
        for i in 0..block_count {
            println!("Requesting block {} from peer {}", i, self.peer);

            // create a message
            let request_message = Message::new(
                MessageID::Request,
                vec![
                    (piece_index as u32).to_be_bytes().to_vec(),
                    (BLOCK_SIZE * i).to_be_bytes().to_vec(),
                    BLOCK_SIZE.to_be_bytes().to_vec()
                ].concat()
            );

            println!("Request message: {:?}", request_message);

            // send request
            if let Err(e) = self.send(stream, request_message).await {
                eprintln!("Error: couldn't send request message to peer {}: {}", self.peer, e);
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Couldn't send request message to peer")); 
            }

            // receive piece
            let mut request_response = self.recv(stream, MessageID::Piece).await?;

            loop {
                if request_response.id == MessageID::Piece {
                    break;
                }

                request_response = self.recv(stream, MessageID::Piece).await?;
            }

            println!("Received block {} from peer {}", i, self.peer);

            let payload = &request_response.payload;

            // check the index
            let piece_index_response = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
            if piece_index_response != piece_index as u32 {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Piece index doesn't match the requested piece index"));
            }

            // check offset start
            let offset_start_response = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
            if offset_start_response != BLOCK_SIZE * i {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Offset start doesn't match the requested offset start"));
            }

            // save the block to piece
            let block = &payload[8..];
            piece.append(&mut block.to_vec());
        }

        // last piece
        let last_block_size: u32 = piece_length as u32 % BLOCK_SIZE;

        if last_block_size == 0 {
            return Ok(piece);
        }

        // create a message
        let request_message = Message::new(
            MessageID::Request,
            vec![
                (piece_index as u32).to_be_bytes().to_vec(),
                (block_count * BLOCK_SIZE).to_be_bytes().to_vec(),
                last_block_size.to_be_bytes().to_vec()
            ].concat()
        );

        println!("Requesting last piece from peer {}", self.peer);

        // send request
        if let Err(e) = self.send(stream, request_message).await {
            eprintln!("Error: couldn't send request message to peer {}: {}", self.peer, e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Couldn't send request message to peer")); 
        }

        // receive piece
        let request_response = self.recv(stream, MessageID::Piece).await?;

        println!("Received last block from peer {}", self.peer);

        let payload = &request_response.payload;

        // check the index
        let piece_index_response = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
        if piece_index_response != piece_index as u32 {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Piece index doesn't match the requested piece index"));
        }

        // check offset start
        let offset_start_response = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
        println!("offset response: {offset_start_response}");
        if offset_start_response != block_count * BLOCK_SIZE {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Offset start doesn't match the requested offset start"));
        }

        // save the block to piece
        let block = &payload[8..];

        // println!("Block: {:?}", block);

        // println!("piece: {:?}", piece);

        piece.append(&mut block.to_vec());

        // check piece hash
        // sha1 hash the piece 

        let piece_hash = sha1_hash(piece.clone());
        if let Some(hash) = self.get_piece_hash(piece_index).await {
            println!("Piece hash: {:?}", piece_hash);
            println!("Hash: {:?}", hash);
            if hash[..] != piece_hash.as_bytes()[..] {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Piece hash doesn't match the requested piece hash"));
            }
        }
        else {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Couldn't get piece hash"));
        }

        Ok(piece)
    }

    pub async fn run(mut self) {
        // ---------------------------- establish connection ---------------------------- 

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
                println!("Connected to peer {}: {:?}", self.peer, stream);
                stream
            },
            Err(e) => {
                eprintln!("Couldn't connect to peer {}: {}", self.peer, e);
                return;
            }
        };
        
        // ---------------------------- send messages ---------------------------- 

        // send a handshake
        self.send_handshake(&mut stream).await;

        // receive handshake response
        if let Err(e) = self.recv_handshake(&mut stream).await {
            eprintln!("Error: couldn't receive handshake from peer {}: {}", self.peer, e);
            return;
        }

        // receive bitfield
        let bitfield_message = match self.recv(&mut stream, MessageID::Bitfield).await {
            Ok(message) => message,
            Err(e) => {
                eprintln!("Error: couldn't receive bitfield message from peer {}: {}", self.peer, e);
                return;
            }
        };

        println!("Bitfield message from peer {}: {:?}", self.peer, bitfield_message);

        // send interested message
        let interested_message = Message::new(
            MessageID::Interested,
            vec![]
        );

        if let Err(e) = self.send(&mut stream, interested_message).await {
            eprintln!("Error: couldn't send interested message to peer {}: {}", self.peer, e);
            return;
        } else {
            self.peer.am_interested = true;
        }

        println!("Interested message sent to peer {}", self.peer);


        // receive unchoke response
        let unchoke_response = match self.recv_response(&mut stream).await {
            Ok(response) => {
                self.peer.choking = false;
                response
            }
            Err(e) => {
                eprintln!("Error: couldn't receive unchoke message response from peer {}: {}", self.peer, e);
                return;
            } 
        };

        if unchoke_response.is_empty() {
            println!("Unchoke response is empty from peer {}", self.peer);
            return;
        }

        println!("Unchoke message response from peer {}: {:?}", self.peer, unchoke_response);

        if self.peer.choking || !self.peer.am_interested {
            todo!("Handle choke and not interested");
        }

        // request piece
        // get random not downloaded piece

        let files_to_download = self.get_files().await;

        while self.pieces_left().await {
            let piece_index = self.get_random_not_downloaded_piece().await;

            let piece = match self.request_piece(&mut stream, piece_index).await {
                Ok(piece) => piece,
                Err(e) => {
                    eprintln!("Error: couldn't request piece from peer {}: {}", self.peer, e);
                    return;
                }
            };
            
            // write to file
            self.write_to_file(piece, piece_index, &files_to_download).await;
            println!("Piece {} written to file", piece_index);
        }

        println!("Finished downloading from peer {}", self.peer);
    }
    
}