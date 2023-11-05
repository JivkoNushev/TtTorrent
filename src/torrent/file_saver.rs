use tokio::{sync::mpsc, io::AsyncWriteExt};

#[derive(Debug)]
pub struct FileSaver {
    file_name: String,
    file_queue: mpsc::Receiver<Vec<u8>>,
}

impl FileSaver {
    pub fn new(file_name: &str, file_queue_rx: mpsc::Receiver<Vec<u8>>) -> FileSaver {
        FileSaver { file_name: file_name.to_string(), file_queue: file_queue_rx }
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
