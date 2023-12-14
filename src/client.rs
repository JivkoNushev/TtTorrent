use lazy_static::lazy_static;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{self, Sender, Receiver};

pub mod downloader;
use downloader::Downloader;

pub mod seeder;
use seeder::Seeder;

pub mod messager;
pub use messager::{ InterProcessMessage, MessageType };

lazy_static! {
    pub static ref CLIENT_PEER_ID: Mutex<[u8;20]> = Mutex::new(Client::create_client_peer_id());
}

pub struct Client {
    downloader: Downloader,
    downloader_tx: mpsc::Sender<InterProcessMessage>,
    seeder: Seeder,
    _seeder_tx: mpsc::Sender<InterProcessMessage>,

    rx: mpsc::Receiver<InterProcessMessage>,
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
    pub fn new(tx: Sender<InterProcessMessage>, rx: Receiver<InterProcessMessage>) -> Client {
        // creating the channels for comunication between processes
        let (downloader_tx, downloader_rx) = mpsc::channel::<InterProcessMessage>(100);
        let (_seeder_tx, _seeder_rx) = mpsc::channel::<InterProcessMessage>(100);
        
        Client { 
            downloader: Downloader::new(tx, downloader_rx),
            downloader_tx,
            seeder: Seeder {},
            _seeder_tx,

            rx,
        }
    }

    fn print_downloaded_percentage(msg: &InterProcessMessage) {
        let downloaded_pieces_count = u32::from_be_bytes(msg.payload[0..4].try_into().unwrap());
        let total_pieces_count = u32::from_be_bytes(msg.payload[4..8].try_into().unwrap());

        let percentage = (downloaded_pieces_count as f32 / total_pieces_count as f32) * 100.0;
        let percentage = (percentage * 100.0).round() / 100.0;

        println!("{}: {}%", msg.torrent_name, percentage);
    }

    pub fn setup_graceful_shutdown(client_tx: Sender<InterProcessMessage>) {
        use tokio::signal::unix::SignalKind;

        // Set up a Ctrl+C signal handler (SIGINT)
        let ctrl_c = tokio::signal::ctrl_c();

        // Set up a termination signal handler (SIGTERM)
        let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate()).unwrap();
        let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt()).unwrap();
        let mut sigquit = tokio::signal::unix::signal(SignalKind::quit()).unwrap();

        // Spawn an async task to handle the signals
        tokio::spawn(async move {
            tokio::select! {
                _ = ctrl_c => {
                    // Handle Ctrl+C (SIGINT)
                    println!("Ctrl+C received. Cleaning up...");
                },
                _ = sigterm.recv() => {
                    // Handle SIGTERM
                    println!("SIGTERM received. Cleaning up...");
                },
                _ = sigint.recv() => {
                    // Handle SIGINT
                    println!("SIGINT received. Cleaning up...");
                },
                _ = sigquit.recv() => {
                    // Handle SIGQUIT
                    println!("SIGQUIT received. Cleaning up...");
                }
            }

            // Perform cleanup or graceful shutdown logic here
            let _ = client_tx.send(
                InterProcessMessage::new(
                    MessageType::SaveState, 
                    String::new(), 
                    Vec::new())
                ).await;
        });
    }

    pub async fn run(mut self) {
        let _ = tokio::join!(
            self.downloader.run(),
            self.seeder.run(),
            tokio::spawn(async move {
                let mut downloader_finished = false;
                let mut seeder_finished = true;

                while let Some(msg) = self.rx.recv().await {
                    match msg.message_type {
                        MessageType::DownloadTorrent => {
                            if let Err(e) = self.downloader_tx.send(msg.clone()).await {
                                println!("Error sending message to downloader: {}", e);
                            }
                        },
                        MessageType::DownloadedPiecesCount => {
                            Client::print_downloaded_percentage(&msg);
                        },
                        MessageType::SaveState => {
                            self.downloader_tx.send(msg.clone()).await.unwrap();
                            // self.seeder_tx.send(msg.clone()).await.unwrap();
                        },
                        MessageType::DownloaderFinished => {
                            downloader_finished = true;
                        },
                        // MessageType::SeederFinished => {
                        //     seeder_finished = true;
                        // },
                        _ => {
                            println!("Unknown message type: {:?}", msg.message_type);
                        }
                    }

                    if downloader_finished && seeder_finished {
                        std::process::exit(0);
                    }
                }
            }),
        );
    }
}
