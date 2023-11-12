use tokio::sync::mpsc;


#[derive(Debug, Clone)]
pub struct TorrentWriter {
    torrent_name: String,
    tx: mpsc::Sender<String>,   
}

impl TorrentWriter {
    pub async fn new(torrent_name: String) -> (TorrentWriter, mpsc::Receiver<String>) {
        let (tx, rx) = mpsc::channel::<String>(100);

        (
            TorrentWriter{
                torrent_name,
                tx
            },
            rx
        )
    }

    pub async fn get_torrent_tx(&self) ->  mpsc::Sender<String> {
        self.tx.clone()
    }

    pub async fn get_name(&self) -> String {
        self.torrent_name.clone()
    }

}

pub struct TorrentWriterHandle {
    torrent_name: String,
    rx: mpsc::Receiver<String>,   
}

impl TorrentWriterHandle {
    pub async fn new(torrent_name: String, rx: mpsc::Receiver<String>) -> TorrentWriterHandle {
        TorrentWriterHandle{
            torrent_name,
            rx
        }
    }

    pub async fn run(mut self) {
        tokio::spawn(async move {
            while let Some(value) = self.rx.recv().await {
                println!("{value:?}")
            }
        });
    }
}

