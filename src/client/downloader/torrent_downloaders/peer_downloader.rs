use tokio::io::{AsyncWriteExt, AsyncReadExt, AsyncSeekExt};
use tokio::sync::{mpsc, Mutex};
use tokio::net::TcpStream;
use std::{sync::Arc, collections::BTreeMap};

use crate::torrent::{Torrent, BencodedValue};
use crate::peer::Peer;
use crate::peer::peer_messages::{Message, MessageID, Handshake};
use crate::client::{Client, InterProcessMessage};
use crate::utils::{sha1_hash, AsBytes, rand_number_u32};

#[derive(Debug)]
pub struct DownloadableFile {
    start: u64,
    size: u64,
    path: String,

    // should be 32 HEX characters
    _md5sum: Option<Vec<u8>>
}

pub struct PeerDownloader {
    pub peer_id: [u8;20],
    pub handler_rx: mpsc::Receiver<InterProcessMessage>
}

impl PeerDownloader {
    pub fn new(peer_id: [u8;20], handler_rx: mpsc::Receiver<InterProcessMessage>) -> PeerDownloader {
        PeerDownloader {
            peer_id,
            handler_rx
        }
    } 
}

pub struct PeerDownloaderHandler {
    peer: Peer,
    torrent: Arc<Mutex<Torrent>>,
    downloader_tx: mpsc::Sender<InterProcessMessage>,
}

// getters
impl PeerDownloaderHandler {
    async fn get_info_hash(&self) -> [u8; 20] {
        let torrent_guard = self.torrent.lock().await;
        torrent_guard.info_hash.as_bytes().clone()
    }

    async fn get_piece_length(&self, piece_index: usize) -> u64 {
        let torrent_guard = self.torrent.lock().await;
        torrent_guard.get_piece_length(piece_index)
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

    async fn get_random_not_downloaded_piece(&self, bitfield: Vec<usize>) -> isize {
        let mut torrent_guard = self.torrent.lock().await;
        
        // piece indexes that are needed from the client and the peer has them
        let common_indexes = bitfield
            .iter()
            .filter(|&i| torrent_guard.pieces_left.contains(i))
            .collect::<Vec<&usize>>();

        if common_indexes.is_empty() {
            return -1;
        }

        // let piece_index = rand::thread_rng().gen_range(0..common_indexes.len());

        let piece_index = rand_number_u32(0, common_indexes.len() as u32) as usize;

        let piece = common_indexes[piece_index];
        
        torrent_guard.pieces_left.remove(piece_index);

        // send a DownloadedPiecesCount message to the downloader
        let pieces_count = torrent_guard.get_pieces_count();
        let downloaded_pieces_count = pieces_count - torrent_guard.pieces_left.len();

        // payload should be 4 be bytes for downloaded and 4 be bytes for total pieces count
        let payload = [
            (downloaded_pieces_count as u32).to_be_bytes().to_vec(),
            (pieces_count as u32).to_be_bytes().to_vec()
        ].concat();

        let message = InterProcessMessage::new(
            crate::client::messager::MessageType::DownloadedPiecesCount,
            torrent_guard.torrent_name.clone(),
            payload
        );

        if let Err(e) = self.downloader_tx.send(message).await {
            panic!("Error: couldn't send DownloadedPiecesCount message to downloader: {}", e)
        }

        piece.clone() as isize
    }

    async fn get_files_to_download(&self) -> Vec<DownloadableFile> {
        let files_to_download = self.get_files().await;

        let mut files_to_download = files_to_download
            .iter()
            .map(|file| {
                let size = match file.get("length") {
                    Some(BencodedValue::Integer(size)) => *size as u64,
                    _ => panic!("Error: couldn't get file size")
                };

                let path = match file.get("path") {
                    Some(BencodedValue::List(path_list)) => {
                        path_list
                            .iter()
                            .map(|path| {
                                match path {
                                    BencodedValue::ByteString(path) => String::from_utf8(path.to_vec()).unwrap(),
                                    _ => panic!("Error: couldn't get file path")
                                }
                            })
                            .collect::<Vec<String>>()
                    },
                    _ => panic!("Error: couldn't get file path")
                };

                let path = path.join("/");

                let _md5sum = match file.get("md5sum") {
                    Some(BencodedValue::ByteString(md5sum)) => Some(md5sum.clone()),
                    _ => None
                };

                DownloadableFile {
                    start: 0,
                    size,
                    path,
                    _md5sum
                }
            })
            .collect::<Vec<DownloadableFile>>();

        // start of file is the size of the previous file
        for i in 1..files_to_download.len() {
            files_to_download[i].start = files_to_download[i - 1].start + files_to_download[i - 1].size;
        }
        
        files_to_download
    }
}

// Peer Messaging methods
impl PeerDownloaderHandler {
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

    async fn handshake(&mut self, stream: &mut TcpStream) -> std::io::Result<()> {
        // send a handshake
        self.send_handshake(stream).await?;

        // receive handshake response
        self.recv_handshake(stream).await?;

        Ok(())
    }

    async fn bitfield(&mut self, stream: &mut TcpStream) -> std::io::Result<Vec<usize>> {
        let bitfield_message = self.recv(stream).await?;
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
        let unchoke_message = self.recv(stream).await?;

        if unchoke_message.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Peer is not unchoking"));
        }

        self.peer.choking = false;

        Ok(())
    }

    async fn request_and_receive_piece(
        &mut self,
        stream: &mut TcpStream,
        piece_index: usize,
        offset_start: u32,
        block_size: u32,
        piece: &mut Vec<u8>,
    ) -> std::io::Result<()> {
        let request_message = Message::new(
            MessageID::Request,
            vec![
                (piece_index as u32).to_be_bytes().to_vec(),
                offset_start.to_be_bytes().to_vec(),
                block_size.to_be_bytes().to_vec(),
            ]
            .concat(),
        );
    
        if let Err(e) = self.send(stream, request_message).await {
            eprintln!("Error: couldn't send request message to peer {}: {}", self.peer, e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Couldn't send request message to peer"));
        }
    
        loop {
            let request_response = self.recv(stream).await?;
            let request_message = Message::from_bytes(request_response);
    
            if request_message.id == MessageID::Piece {
                let payload = &request_message.payload;
    
                let expected_index = piece_index as u32;
                let expected_offset = offset_start;
    
                let piece_index_response = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                let offset_start_response = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
    
                if piece_index_response == expected_index && offset_start_response == expected_offset {
                    let block = &payload[8..];
                    piece.extend_from_slice(block);
                    break;
                }
            }
        }
    
        Ok(())
    }

    async fn request_piece(&mut self, stream: &mut TcpStream, piece_index: usize) -> std::io::Result<Vec<u8>> {
        const BLOCK_SIZE: u32 = 1 << 14;
    
        let piece_length = self.get_piece_length(piece_index).await as u32;
        let block_count = piece_length / BLOCK_SIZE;
    
        let mut piece = Vec::new();
    
        for i in 0..block_count {
            if let Err(e) = self.request_and_receive_piece(stream, piece_index, i * BLOCK_SIZE, BLOCK_SIZE, &mut piece).await {
                return Err(e);
            }
        }
    
        let last_block_size = piece_length % BLOCK_SIZE;
    
        if last_block_size != 0 {
            if let Err(e) = self.request_and_receive_piece(stream, piece_index, block_count * BLOCK_SIZE, last_block_size, &mut piece).await {
                return Err(e);
            }
        }
    
        let piece_hash = sha1_hash(piece.clone());
        if let Some(hash) = self.get_piece_hash(piece_index).await {
            if hash[..] != piece_hash.as_bytes()[..] {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Piece hash doesn't match the requested piece hash"));
            }
        } else {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Couldn't get piece hash"));
        }
    
        Ok(piece)
    }
}

// Torrent methods
impl PeerDownloaderHandler {
    async fn pieces_left(&self) -> bool {
        let torrent_guard = self.torrent.lock().await;
        !torrent_guard.pieces_left.is_empty()
    }
    
    fn check_hash(l: &[u8;20], r: &[u8;20]) -> bool {
        l == r
    }
}

impl PeerDownloaderHandler {
    pub fn new(peer: Peer, torrent: Arc<Mutex<Torrent>>, downloader_tx: mpsc::Sender<InterProcessMessage>) -> PeerDownloaderHandler {
        PeerDownloaderHandler {
            peer,
            torrent,
            downloader_tx
        }
    }

    async fn write_to_file(&self, piece: Vec<u8>, piece_index: usize, files: &Vec<DownloadableFile>) {
        let piece_length = self.get_piece_length(piece_index).await;
        let mut piece_start_offset = self.get_piece_length(0).await * piece_index as u64;

        let mut file_index= 0;
        let mut file_offset = 0;
        
        let mut bytes_left = piece_length;
        while bytes_left > 0 {
            for (i, file) in files.iter().enumerate() {
                // if this piece is in this file
                if file.start <= piece_start_offset && piece_start_offset < file.start + file.size {
                    file_index = i;
                    file_offset = piece_start_offset - file.start;
    
                    break;
                }
            }  
            
            let file = &files[file_index];

            let torrent_dest_path = self.torrent.lock().await.get_dest_path().clone();

            let file_path = std::path::PathBuf::from(format!("{}/{}", torrent_dest_path, file.path));
            
            // create the directories if they don't exist
            let file_dir = file_path.parent().unwrap();
            tokio::fs::create_dir_all(file_dir).await.unwrap();

            let mut fd = tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(file_path.clone())
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
            bytes_left -= bytes_to_write;
            file_index += 1;
        }
    }

    async fn send(&mut self, stream: &mut TcpStream, message: Message) -> std::io::Result<()> {
        stream.write_all(&message.as_bytes()).await?;
        Ok(())
    }

    async fn recv(&mut self, stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
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

    pub async fn run(mut self) -> std::io::Result<()> {
        // ---------------------------- establish connection ---------------------------- 
        
        let mut stream = self.get_stream().await?;

        // ---------------------------- send messages ---------------------------- 
        
        // _____________send handshake_____________
        self.handshake(&mut stream).await?;

        // _____________receive bitfield_____________
        let available_pieces = self.bitfield(&mut stream).await?;
        
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
            if piece_index == -1 {
                break;
            }
            let piece_index = piece_index as usize;

            let piece = self.request_piece(&mut stream, piece_index).await?;
            
            self.write_to_file(piece, piece_index, &files).await;
        }

        println!("Peer '{}' doesn't have more pieces", self.peer);

        Ok(())
    }
    
}