use byteorder::{BigEndian, ByteOrder};
use tokio::{sync::mpsc, io::{AsyncWriteExt, AsyncReadExt}};

use crate::torrent::tracker::Tracker;
use crate::utils::print_as_string;
    
pub mod peer_connection;
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
    
#[derive(Debug, Clone)]
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
    pub fn new(peer_address: PeerAddress, id_num: usize, file_queue_tx: mpsc::Sender<Vec<u8>>) -> Peer {
        Peer { 
            id: peer_create_id(id_num), 
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
        println!("size:{} Handshake request for peer {}: {:?}", handshake.len(), self.address, handshake);
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
        println!("Handshake response for peer {}: {:?}", self.address, handshake_response);
        self.set_id(handshake_response[48..].try_into().unwrap());
        println!("Peer {} ID added: {:?}", self.address, self.id);
    }

    async fn request_hash(index: u32, stream: &mut tokio::net::TcpStream, tracker: Tracker) {
        let size: u32 = 13;
        let id: u8 = 6;
        // get the the piece size
        // let piece_size = tracker.get_torrent_file().get_piece_size();
        
        let mut message: Vec<u8> = Vec::new();
        message.append(&mut size.to_be_bytes().to_vec());
        message.push(id);
        message.append(&mut index.to_be_bytes().to_vec());
        message.append(&mut vec![0 as u8; 4]);
    }

    pub async fn message(&mut self, stream: &mut tokio::net::TcpStream, tracker: Tracker) {
        // start exchanging messages
        // send interested message
        let size: u32 = 1;
        let mut message = Vec::new();
        message.append(&mut size.to_be_bytes().to_vec());
        message.push(2);

        stream.write_all(&message).await.unwrap();
        println!("Interested in peer {} : {:?}", self.address, message);

        // receive interested message
        let mut interested_response_length = vec![0; 4];
        stream.read_exact(&mut interested_response_length).await.unwrap();

        let size = BigEndian::read_u32(&interested_response_length);
        let mut interested_response = vec![0; size as usize];
        stream.read_exact(&mut interested_response).await.unwrap();
        println!("Interested response from peer {}: {:?}", self.address, interested_response);
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

        println!("Peer {} finished downloading", self.address)
    }
}