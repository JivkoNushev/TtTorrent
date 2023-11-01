use byteorder::{BigEndian, ByteOrder};
use tokio::{sync::mpsc, io::{AsyncWriteExt, AsyncReadExt}};

use crate::{torrent_file::Sha1Hash, utils::print_as_string, tracker::Tracker};


pub mod peer_connection;
pub mod peer_messages;

pub use peer_connection::{peer_create_id, get_peers};


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerAddress {
    pub address: String,
    pub port: String
}
impl PeerAddress {
    pub fn new(peer_address: [u8;6]) -> PeerAddress {
        let mut address = String::new();
        for i in &peer_address[..4] {
            address.push_str(&i.to_string());
            address.push('.');
        }
        address.pop();
        let port = BigEndian::read_u16(&peer_address[4..]).to_string();        

        PeerAddress{address, port}
    }
}
    
#[derive(Debug)]
pub struct Peer {
    id: [u8;20],
    address: String,
    port: String,
    am_choking: bool,
    am_interested: bool,
    peer_choking: bool,
    peer_interested: bool,
    file_queue: mpsc::Sender<Vec<u8>>,
}
impl Peer {
    pub fn new(peer_address: PeerAddress, id: [u8;20], file_queue_tx: mpsc::Sender<Vec<u8>>) -> Peer {
        Peer { 
            id: id, 
            address: peer_address.address,
            port: peer_address.port,
            am_choking: false, 
            am_interested: false, 
            peer_choking: true, 
            peer_interested: false,
            file_queue: file_queue_tx,
        }
    }

    pub fn set_id(&mut self, id: [u8;20]) {
        self.id = id;
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

    async fn handshake(&mut self, stream: &mut tokio::net::TcpStream, tracker: Tracker) {
        // create handshake
        let mut handshake: Vec<u8> = Vec::new();
        handshake.push(19); // len of the protocol
        handshake.append(&mut "BitTorrent protocol".as_bytes().to_vec());
        handshake.append(&mut vec![0 as u8; 8]);
        handshake.append(&mut tracker.get_hashed_info_dict().get_hash_ref().to_vec());
        handshake.append(&mut tracker.get_id().as_bytes().to_vec());

        // send handshake
        stream.write_all(&handshake).await.unwrap();
        println!("size:{} Handshake request: {:?}", handshake.len(), handshake);
        print_as_string(&handshake);

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
        self.set_id(handshake_response[48..].try_into().unwrap());
        println!("Peer ID: {:?}", self.id);
    }

    pub async fn message(&mut self, stream: &mut tokio::net::TcpStream, tracker: Tracker) {
        // start exchanging messages
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
        println!("after sleep");
    }

    pub async fn download(&mut self, tracker: Tracker) {
        // create a tcp connection to the peer
        let mut stream = tokio::net::TcpStream::connect(format!("{}:{}", self.address, self.port)).await;

        match stream {
            Ok(mut stream) => {
                println!("Connected to peer: {}", self.address);
                self.handshake(&mut stream, tracker.clone()).await;
                self.message(&mut stream, tracker.clone()).await
            },
            Err(e) => {
                println!("Error connecting to peer:{:?}: {}", self.id, e);
            }
        }
    }
}