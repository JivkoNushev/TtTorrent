use std::{sync::Arc, fmt::Result, f32::consts::E, mem, collections::BTreeMap, ops::Index};

use sha1::digest::typenum::bit;
use tokio::{sync::{mpsc, Mutex}, fs::File, net::TcpStream, io::{AsyncWriteExt, AsyncReadExt, AsyncSeekExt}};
use rand::Rng;

use crate::{torrent::{Torrent, Sha1Hash, BencodedValue}, peer::peer_messages::{Message, MessageID, Handshake}, utils::AsBytes, client::{CLIENT_PEER_ID, Client}};
use crate::peer::Peer;
use crate::utils::sha1_hash;

#[derive(Debug)]
pub struct DownloadableFile {
    start: u64,
    size: u64,
    path: String,
    md5sum: Option<String>
}

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
    _downloader_tx: mpsc::Sender<String>,
}

// getters
impl PeerDownloaderHandler {
    async fn get_info_hash(&self) -> [u8; 20] {
        let torrent_guard = self.torrent.lock().await;

        torrent_guard.info_hash.as_bytes().clone()
    }

    async fn get_piece_length(&self, piece_index: usize) -> u32 {
        let torrent_guard = self.torrent.lock().await;

        torrent_guard.get_piece_length(piece_index) as u32
    }

    async fn get_piece_hash(&self, piece_index: usize) -> Option<[u8; 20]> {
        let torrent_guard = self.torrent.lock().await;

        torrent_guard.torrent_file
            .get_piece_hash(piece_index)
            .map(|piece| piece.as_bytes().to_owned())
    }

    async fn get_files(&self) -> Vec<BTreeMap<String, BencodedValue>> {
        let torrent_guard = self.torrent.lock().await;

        torrent_guard.get_files()
    }
}

impl PeerDownloaderHandler {
    pub fn new(peer: Peer, torrent: Arc<Mutex<Torrent>>, _downloader_tx: mpsc::Sender<String>) -> PeerDownloaderHandler {
        PeerDownloaderHandler {
            peer,
            torrent,
            _downloader_tx
        }
    }

    async fn get_random_not_downloaded_piece(&self, bitfield: Vec<usize>) -> usize {
        
        let mut torrent_guard = self.torrent.lock().await;
        
        // piece indexes that are needed from the client and the peer has them
        let common_indexes = bitfield
            .iter()
            .filter(|&i| torrent_guard.pieces_left.contains(i))
            .collect::<Vec<&usize>>();

        let piece_index = rand::thread_rng().gen_range(0..common_indexes.len());
        let piece = common_indexes[piece_index];
        
        torrent_guard.pieces_left.remove(piece_index);

        piece.clone()
    }

    async fn pieces_left(&self) -> bool {
        let torrent_guard = self.torrent.lock().await;

        !torrent_guard.pieces_left.is_empty()
    }

    async fn get_files_to_download(&self) -> Vec<DownloadableFile> {
        let files_to_download = self.get_files().await;
        // println!("got files: {}", files_to_download.len());

        let mut files_to_download = files_to_download.iter().map(|file| {
            let size = if let BencodedValue::Integer(size) = file.get("length").unwrap() {
                *size as u64
            } else {
                panic!("Error: couldn't get file size");
            };

            let path: Vec<String> = if let BencodedValue::List(path) = file.get("path").unwrap() {
                path.iter().map(|path| {
                    if let BencodedValue::String(path) = path {
                        path.clone()
                    } else {
                        panic!("Error: couldn't get file path");
                    }
                }).collect()
            } else {
                panic!("Error: couldn't get file path");
            };
            let path = path.join("/");

            let md5sum = if let Some(BencodedValue::String(md5sum)) = file.get("md5sum") {
                Some(md5sum.clone())
            } else {
                None
            };

            DownloadableFile {
                start: 0,
                size,
                path,
                md5sum
            }
        }).collect::<Vec<DownloadableFile>>();

        // start of file is the size of the previous file
        for i in 1..files_to_download.len() {
            files_to_download[i].start = files_to_download[i - 1].start + files_to_download[i - 1].size;
        }
        
        files_to_download
    }

    async fn write_to_file(&self, piece: Vec<u8>, piece_index: usize, files: &Vec<DownloadableFile>) {
        let piece_length = self.get_piece_length(piece_index).await as u64;

        let mut piece_start_offset = self.get_piece_length(0).await as u64 * piece_index as u64;

        let mut file_index= 0;
        let mut file_offset = 0;
        
        let mut bytes_left = piece_length;
        
        while bytes_left > 0 {
            let mut changed = false;
            for (i, file) in files.iter().enumerate() {
                if file.start <= piece_start_offset && piece_start_offset < file.start + file.size {
                    file_index = i as u64;
                    file_offset = piece_start_offset - file.start;
    
                    changed = true;
    
                    break;
                }
            }  
    
            if changed == false {
                eprintln!("Error: couldn't find file index and offset");
                return
            }
            
            let file = &files[file_index as usize];

            let path_buf = std::path::PathBuf::from(format!("test_data/folder_with_files/{}", file.path));

            let dir = path_buf.parent().unwrap();

            tokio::fs::create_dir_all(dir).await.unwrap();

            let mut fd = tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                // .append(true)         BRUH
                .create(true)
                .open(path_buf.clone())
                .await
                .unwrap();

            let bytes_to_write = if file.size - file_offset > bytes_left  {
                bytes_left
            } else {
                file.size - file_offset
            };
            
            fd.seek(std::io::SeekFrom::Start(file_offset)).await.unwrap();

            let written_bytes = piece_length - bytes_left;

            fd.write_all(
                &piece[written_bytes as usize..(written_bytes + bytes_to_write) as usize]
            ).await.unwrap();

            piece_start_offset += bytes_to_write;

            bytes_left -= bytes_to_write as u64;
            file_index += 1;
        }
    }

    fn check_hash(l: &[u8;20], r: &[u8;20]) -> bool {
        l == r
    }

    async fn send(&mut self, stream: &mut TcpStream, message: Message) -> std::io::Result<()> {
        stream.write_all(&message.as_bytes()).await?;
        Ok(())
    }

    async fn recv_response(&mut self, stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
        let mut recv_size: [u8; 4] = [0; 4];

        let mut recv_size_u32: u32;
        loop {
            stream.read_exact(&mut recv_size).await?;

            recv_size_u32 = u32::from_be_bytes(recv_size);

            if 0 != recv_size_u32 {
                break;
            }
        }
        
        let mut buf: Vec<u8> = vec![0; recv_size_u32 as usize];

        stream.read_exact(&mut buf).await?;

        Ok(buf)
    }

    async fn send_handshake(&mut self, stream: &mut TcpStream) -> std::io::Result<()> {
        let handshake = Handshake::new( 
            self.get_info_hash().await, 
            Client::get_client_id().await
        );

        stream.write_all(&handshake.as_bytes()).await
    }

    async fn recv_handshake(&mut self, stream: &mut TcpStream) -> tokio::io::Result<[u8;20]> {
        let mut buf = vec![0; 68];

        stream.read_exact(&mut buf).await.map_err(|e| {
            tokio::io::Error::new(tokio::io::ErrorKind::Other, format!("Error: couldn't receive handshake response from peer {}: {}", self.peer, e))
        })?;

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

        Ok(handshake.peer_id)
    }

    async fn request_piece(&mut self, stream: &mut TcpStream, piece_index: usize) -> std::io::Result<Vec<u8>> {
        let piece_length: u32 = self.get_piece_length(piece_index).await as u32;

        const BLOCK_SIZE: u32 = 1 << 14;

        let block_count: u32 = piece_length / BLOCK_SIZE;

        let mut piece: Vec<u8> = Vec::new();
        
        for i in 0..block_count {
            // create a message
            let request_message = Message::new(
                MessageID::Request,
                vec![
                    (piece_index as u32).to_be_bytes().to_vec(),
                    (BLOCK_SIZE * i).to_be_bytes().to_vec(),
                    BLOCK_SIZE.to_be_bytes().to_vec()
                ].concat()
            );

            // send request
            if let Err(e) = self.send(stream, request_message).await {
                eprintln!("Error: couldn't send request message to peer {}: {}", self.peer, e);
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Couldn't send request message to peer")); 
            }

            // receive piece
            let request_response = self.recv_response(stream).await?;
            let mut request_message = Message::from_bytes(request_response);

            loop {
                if request_message.id == MessageID::Piece {
                    break;
                }

                let request_response = self.recv_response(stream).await?;
                request_message = Message::from_bytes(request_response);
            }

            let payload = &request_message.payload;

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

        // send request
        if let Err(e) = self.send(stream, request_message).await {
            eprintln!("Error: couldn't send request message to peer {}: {}", self.peer, e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Couldn't send request message to peer")); 
        }

        // receive piece
        let request_response = self.recv_response(stream).await?;
        let mut request_message = Message::from_bytes(request_response);

        loop {
            if request_message.id == MessageID::Piece {
                break;
            }

            let request_response = self.recv_response(stream).await?;
            request_message = Message::from_bytes(request_response);
        }

        let payload = &request_message.payload;

        // check the index
        let piece_index_response = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
        if piece_index_response != piece_index as u32 {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Piece index doesn't match the requested piece index"));
        }

        // check offset start
        let offset_start_response = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
        if offset_start_response != block_count * BLOCK_SIZE {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Offset start doesn't match the requested offset start"));
        }

        // save the block to piece
        let block = &payload[8..];

        piece.append(&mut block.to_vec());

        let piece_hash = sha1_hash(piece.clone());
        if let Some(hash) = self.get_piece_hash(piece_index).await {
            if hash[..] != piece_hash.as_bytes()[..] {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Piece hash doesn't match the requested piece hash"));
            }
        }
        else {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Couldn't get piece hash"));
        }

        Ok(piece)
    }

    async fn get_stream(&mut self) -> std::io::Result<TcpStream> {
        let mut stream = TcpStream::connect(format!("{}:{}", self.peer.address, self.peer.port)).await;

        for _ in 0..100 {
            if let Ok(_) = stream {
                break;
            }

            stream = TcpStream::connect(format!("{}:{}", self.peer.address, self.peer.port)).await;
        }

        stream
    }

    async fn handshake(&mut self, stream: &mut TcpStream) -> std::io::Result<()> {
        // send a handshake
        self.send_handshake(stream).await?;

        // receive handshake response
        self.recv_handshake(stream).await?;

        Ok(())
    }

    async fn interested(&mut self, stream: &mut TcpStream) -> std::io::Result<()> {
        let interested_message = Message::new(
            MessageID::Interested,
            vec![]
        );

        self.send(stream, interested_message).await?;
        
        self.peer.am_interested = true;

        Ok(())
    }

    async fn unchoke(&mut self, stream: &mut TcpStream) -> std::io::Result<()> {
        let unchoke_message = self.recv_response(stream).await?;

        if unchoke_message.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Peer is not unchoking"));
        }

        self.peer.choking = false;

        Ok(())
    }

    async fn get_available_pieces(&mut self, stream: &mut TcpStream) -> std::io::Result<Vec<usize>> {
        let bitfield_message = self.recv_response(stream).await?;
        let bitfield_message = Message::from_bytes(bitfield_message);

        if bitfield_message.id != MessageID::Bitfield {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Peer didn't send a bitfield message"));
        }
        
        let mut available_pieces = Vec::new();
        for (i, byte) in bitfield_message.payload.iter().enumerate() {
            for j in 0..8 {
                if byte & (1 << (7 - j)) != 0 {
                    available_pieces.push(i * 8 + j);
                }
            }
        }

        Ok(available_pieces)
    }

    pub async fn run(mut self) -> std::io::Result<()> {
        // ---------------------------- establish connection ---------------------------- 
        
        let mut stream = self.get_stream().await?;

        // ---------------------------- send messages ---------------------------- 
        
        // _____________send handshake_____________
        self.handshake(&mut stream).await?;

        // _____________receive bitfield_____________
        let available_pieces = self.get_available_pieces(&mut stream).await?;
        
        // _____________send interested_____________
        self.interested(&mut stream).await?;

        // _____________receive unchoke_____________
        self.unchoke(&mut stream).await?;

        
        if self.peer.choking || !self.peer.am_interested {
            todo!("Handle choke and not interested");
        }
        // ---------------------------- download ----------------------------

        let files = self.get_files_to_download().await;

        while self.pieces_left().await {
            let piece_index = self.get_random_not_downloaded_piece(available_pieces.clone()).await;
            let piece = self.request_piece(&mut stream, piece_index).await?;
            
            // write to file
            self.write_to_file(piece, piece_index, &files).await;
        }

        println!("Finished downloading from peer {}", self.peer);

        Ok(())
    }
    
}