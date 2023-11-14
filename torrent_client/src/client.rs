
use lazy_static::lazy_static;
use tokio::sync::{mpsc, Mutex};

pub mod downloader;
pub mod seeder;

use downloader::Downloader;
use seeder::Seeder;

lazy_static! {
    pub static ref CLIENT_PEER_ID: Mutex<[u8;20]> = Mutex::new(Client::create_client_peer_id());
}

pub struct Client {
    downloader: Downloader,
    seeder: Seeder
}

impl Client {
    pub async fn new() -> (Client, mpsc::Sender<String>, mpsc::Sender<String>) {
        let (downloader_tx, downloader_rx) = mpsc::channel::<String>(100);
        let (client_tx, _client_rx) = mpsc::channel::<String>(100);
        
        let (seeder_tx, _seeder_rx) = mpsc::channel::<String>(100);
        
        (
            Client { 
                downloader: Downloader::new(client_tx, downloader_rx).await,
                seeder: Seeder {}
            },
            downloader_tx,
            seeder_tx
        )
    }

    pub fn create_client_peer_id() -> [u8; 20] {
        "TtT-1-0-0-TESTCLIENT".as_bytes().try_into().unwrap()
    }

    pub async fn run(self) {
        let _ = tokio::join!(
            self.downloader.run(),
            self.seeder.run()
        );
    }
}
