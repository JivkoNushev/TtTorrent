use lazy_static::lazy_static;
use tokio::sync::{mpsc, Mutex};

pub mod downloader;
use downloader::Downloader;

pub mod seeder;
use seeder::Seeder;

lazy_static! {
    pub static ref CLIENT_PEER_ID: Mutex<[u8;20]> = Mutex::new(Client::create_client_peer_id());
}

pub struct Client {
    downloader: Downloader,
    seeder: Seeder
}

// CLIENT_PEER_ID methods
impl Client {
    fn create_client_peer_id() -> [u8; 20] {
        "TtT-1-0-0-TESTCLIENT".as_bytes().try_into().unwrap()
    }

    pub async fn get_client_id() -> [u8;20] {
        let client_peer_id = CLIENT_PEER_ID.lock().await;
        client_peer_id.clone()
    }
}

// Client methods
impl Client {
    pub fn new() -> (Client, mpsc::Sender<(String, String)>, mpsc::Sender<String>) {
        // creating the channels for comunication between processes
        let (downloader_tx, downloader_rx) = mpsc::channel::<(String, String)>(100);
        let (client_tx, _client_rx) = mpsc::channel::<String>(100);
        let (seeder_tx, _seeder_rx) = mpsc::channel::<String>(100);
        
        (
            Client { 
                downloader: Downloader::new(client_tx, downloader_rx),
                seeder: Seeder {}
            },
            downloader_tx,
            seeder_tx
        )
    }

    pub async fn run(self) {
        let _ = tokio::join!(
            self.downloader.run(),
            self.seeder.run()
        );
    }
}
