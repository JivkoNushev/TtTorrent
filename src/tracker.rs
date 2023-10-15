use std::num::ParseIntError;

use crate::torrent_file::{BencodedValue, PeerAddress, parse_to_torrent_file, Sha1Hash, parse_tracker_response};

use reqwest;
use sha1::{Sha1, Digest};
use tokio::io::{AsyncWriteExt, AsyncReadExt};

fn sha1_hash(value: Vec<u8>) -> Sha1Hash {
    let mut hasher = Sha1::new();
    hasher.update(value);
    Sha1Hash::new(&hasher.finalize())
}   

fn create_peer_id(id: String) -> String {
    let padding_len = 20 - id.len();
    let mut peer_id = id.clone();
    let padding: String = vec!['C';padding_len].into_iter().collect();
    peer_id.push_str(&padding);

    peer_id
}

// TODO: Change to Result type insted of Option
fn create_tracker_url(torrent_file: &mut BencodedValue) -> Option<String> {
    if let BencodedValue::String(tracker_announce) = torrent_file.get_from_dict("announce") {
        if let BencodedValue::Dict(info_dict) = torrent_file.get_from_dict("info") {
            let bencoded_info_dict = parse_to_torrent_file(&torrent_file.get_from_dict("info"));
            let hashed_dict_url_encoded = sha1_hash(bencoded_info_dict).as_url_encoded();

            let mut tracker_url = tracker_announce;

            tracker_url.push_str("?info_hash=");
            tracker_url.push_str(&hashed_dict_url_encoded);

            tracker_url.push_str("&peer_id=");
            tracker_url.push_str(create_peer_id("1".to_string()).as_str());

            tracker_url.push_str("&port=6881&uploaded=0&downloaded=0&left=0&compact=1&event=started");
            Some(tracker_url)
        }
        else {
            None
        }
        
    }
    else {
        None
    }
}

pub struct FileSaver {
    file_name: String,
    file_queue: mpsc::Receiver<Vec<u8>>,
}

impl FileSaver {
    pub fn new(file_name: String, file_queue_rx: mpsc::Receiver<Vec<u8>>) -> FileSaver {
        FileSaver { file_name: file_name, file_queue: file_queue_rx }
    }

    pub async fn start(&mut self) {
        loop {
            let piece = self.file_queue.recv().await;

            if let Some(piece) = piece {
                self.save(piece).await;
            }
            else {
                break;
            }
        }
    }

    async fn save(&self, piece: Vec<u8>) {
        let mut file = tokio::fs::File::create(self.file_name.clone()).await.unwrap();
        file.write_all(&piece).await.unwrap(); // ??
    }
}
    
#[derive(Debug)]
pub struct Peer {
    id: String,
    address: String,
    port: String,
    am_choking: bool,
    am_interested: bool,
    peer_choking: bool,
    peer_interested: bool,
    file_queue: mpsc::Sender<Vec<u8>>,
}

use tokio::sync::mpsc;

impl Peer {
    pub fn new(peer_address: PeerAddress, id: String, file_queue_tx: mpsc::Sender<Vec<u8>>) -> Peer {
        Peer { 
            id: id, 
            address: peer_address.get_ip().clone(), 
            port: peer_address.get_port().clone(), 
            am_choking: false, 
            am_interested: false, 
            peer_choking: true, 
            peer_interested: false,
            file_queue: file_queue_tx,
        }
    }
    
    // pub fn choke(&mut self) {
    //     self.am_choking = true;
    // }

    // pub fn unchoke(&mut self) {
    //     self.am_choking = false;
    // }

    // pub fn peer_interest(&mut self) {
    //     self.peer_interested = true;
    // }

    // pub fn peer_uninterest(&mut self) {
    //     self.peer_interested = false;
    // }

    pub fn interest(&mut self) {
        self.am_interested = true;
    }

    pub fn uninterest(&mut self) {
        self.am_interested = false;
    }

    pub fn peer_choke(&mut self) {
        self.peer_choking = true;
    }

    pub fn peer_unchoke(&mut self) {
        self.peer_choking = false;
    }

    async fn handshake(&self, stream: &mut tokio::net::TcpStream, bencoded_info_dict: BencodedValue) {
        // TODO: return a result type for get_from_dict
        let bencoded_info_dict = parse_to_torrent_file(&bencoded_info_dict.get_from_dict("info"));
        let mut hashed_dict_url_encoded = sha1_hash(bencoded_info_dict).get_hash_ref().to_vec();

        // create handshake
        let mut handshake: Vec<u8> = Vec::new();
        handshake.push(19); // len of the protocol
        handshake.append(&mut "BitTorrent protocol".as_bytes().to_vec());
        handshake.append(&mut vec![0 as u8; 8]);
        handshake.append(&mut hashed_dict_url_encoded);
        handshake.append(&mut self.id.as_bytes().to_vec());

        // send handshake
        stream.write_all(&handshake).await.unwrap();

        // receive handshake
        let mut handshake_response = vec![0; 68];
        match stream.read_exact(&mut handshake_response).await {
            Ok(_) => {},
            Err(e) => {
                println!("Error reading handshake response: {:?}", e);
                return;
            }
        }

        println!("Handshake response: {:?}", handshake_response);

        loop {
            let message = vec![0 as u8; 4];
            // send keep alive message
            stream.write_all(&message).await.unwrap();
            println!("Keep alive: {:?}", message);
    
            // receive keep alive message
            let mut keep_alive_response = vec![0; 4];
            stream.read_exact(&mut keep_alive_response).await.unwrap();
    
            println!("Keep alive response: {:?}", keep_alive_response);
    
            // wait 100 seconds
            tokio::time::sleep(tokio::time::Duration::from_secs(100)).await;
        }


    }

    pub async fn download(&self, bencoded_info_dict: BencodedValue) {
        // create a tcp connection to the peer
        loop {
            let mut stream = tokio::net::TcpStream::connect(format!("{}:{}", self.address, self.port)).await;

            match stream {
                Ok(mut stream) => {
                    println!("Connected to peer: {}", self.address);
                    self.handshake(&mut stream, bencoded_info_dict.clone()).await;
                    break;
                },
                Err(e) => {
                    // println!("Error connecting to peer:{}: {}", self.id, e);
                    continue;
                }
            }
        }
        

        // start exchanging messages
    }
}

pub async fn get_peers(torrent_file: &mut BencodedValue, file_queue_tx: mpsc::Sender<Vec<u8>>) -> Vec<Peer> {
    let url = create_tracker_url(torrent_file).unwrap();

    let resp = reqwest::get(url).await.unwrap();

    let bencoded_response = resp.bytes().await.unwrap();

    let bencoded_dict = parse_tracker_response(&bencoded_response);
    
    let mut peer_array: Vec<Peer> = Vec::new();

    if let BencodedValue::Dict(dict) = bencoded_dict {
        if let BencodedValue::ByteAddresses(byte_addresses) = dict.get("peers").unwrap() {
            for i in 0..20 {
                if let Some(addr) = byte_addresses.get(i) {
                    let peer = Peer::new(addr.clone(), i.to_string(), file_queue_tx.clone());
                    peer_array.push(peer);
                }
            }
        }
    }

    peer_array
}
