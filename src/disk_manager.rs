use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::task::JoinHandle;
use tokio::sync::{mpsc, Mutex, Semaphore};
use anyhow::{anyhow, Context, Result};

use std::sync::Arc;

use crate::peer::block_picker::Piece;
use crate::peer::Block;
use crate::messager::ClientMessage;

pub mod torrent_context;
pub use torrent_context::{DiskTorrentContext, DownloadableFile};

pub struct DiskManagerHandle {
    pub tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<()>,
}

impl DiskManagerHandle {
    pub fn new(torrent_context: DiskTorrentContext) -> Self {
        let (tx, rx) = mpsc::channel(100);

        let disk_writer = DiskManager::new(rx, torrent_context);
        let join_handle = tokio::spawn(async move {
            if let Err(e) = disk_writer.run().await {
               tracing::error!("Disk writer error: {:?}", e);
            }
        });

        Self {
            tx,
            join_handle,
        }
    }

    pub async fn join(self) -> Result<()> {
        self.join_handle.await?;
        Ok(())
    }

    pub async fn is_finished(&mut self) -> bool {
        self.join_handle.is_finished()
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.tx.send(ClientMessage::Shutdown).await?;
        Ok(())
    }

    pub async fn write_block(&mut self, block: Block) -> Result<()> {
        self.tx.send(ClientMessage::DownloadedBlock{ block }).await?;
        Ok(())
    }

    pub async fn read_block(&mut self, block: Block, tx: mpsc::Sender<ClientMessage>) -> Result<()> {
        self.tx.send(ClientMessage::Request{ block, tx }).await?;
        Ok(())
    }
}

struct DiskManager {
    rx: mpsc::Receiver<ClientMessage>,
    
    downloaded: Arc<Mutex<Vec<Piece>>>,
    downloaded_pieces_count: Arc<Mutex<usize>>,
    torrent_context: Arc<DiskTorrentContext>,
}

impl DiskManager {
    pub fn new(rx: mpsc::Receiver<ClientMessage>, torrent_context: DiskTorrentContext) -> Self {
        Self {
            rx,
            
            downloaded: Arc::new(Mutex::new(Vec::new())),
            downloaded_pieces_count: Arc::new(Mutex::new(0)),
            torrent_context: Arc::new(torrent_context),
        }
    }

    async fn write_to_file(torrent_context: &DiskTorrentContext, block: Block) -> Result<()> {
        let files = torrent_context.files.clone();

        let data = match block.data {
            Some(data) => data,
            None => {
                tracing::error!("Trying to write block with no data");
                return Err(anyhow!("Trying to write block with no data"));
            }
        };

        let mut block_start = (block.index * torrent_context.torrent_info.piece_length as u32 + block.begin) as u64;

        let mut file_index= 0;
        let mut file_begin = 0;
        
        let mut bytes_left = block.length as u64;
        while bytes_left > 0 {
            for (i, file) in files.iter().enumerate() {
                // if this piece is in this file
                if file.start <= block_start && block_start < file.start + file.size {
                    file_index = i;
                    file_begin = block_start - file.start;
    
                    break;
                }
            }  
            
            let file = &files[file_index];

            let file_path = std::path::Path::new(&torrent_context.dest_path).join(file.path.clone());
            
            // create the directories if they don't exist
            let file_dir = match file_path.parent() {
                Some(file_dir) => file_dir,
                None => return Err(anyhow!("Invalid torrent file path"))
            };

            tokio::fs::create_dir_all(file_dir).await.context("creating the directories").unwrap(); // file_dir is always a directory
            
            let mut fd = tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(file_path)
                .await?;


            let bytes_to_write = if file.size - file_begin > bytes_left  {
                bytes_left
            } else {
                file.size - file_begin
            };
            
            fd.seek(std::io::SeekFrom::Start(file_begin)).await?;

            let written_bytes = block.length as u64 - bytes_left;

            fd.write_all(
                &data[written_bytes as usize..(written_bytes + bytes_to_write) as usize]
            ).await?;

            block_start += bytes_to_write;
            bytes_left -= bytes_to_write;
            file_index += 1;
        }

        Ok(())
    }

    async fn read_block(torrent_context: &DiskTorrentContext, mut block: Block) -> Result<Block> {
        let mut data = Vec::new();

        let files = torrent_context.files.clone();

        let mut block_start = (block.index * torrent_context.torrent_info.piece_length as u32 + block.begin) as u64;

        let mut file_index= 0;
        let mut file_begin = 0;
        
        let mut bytes_left = block.length as u64;
        while bytes_left > 0 {
            for (i, file) in files.iter().enumerate() {
                // if this piece is in this file
                if file.start <= block_start && block_start < file.start + file.size {
                    file_index = i;
                    file_begin = block_start - file.start;
    
                    break;
                }
            }  
            
            let file = &files[file_index];

            let file_path = std::path::PathBuf::from(format!("{}/{}", torrent_context.dest_path, file.path));
            
            let mut fd = tokio::fs::OpenOptions::new()
                .read(true)
                .write(false)
                .create(false)
                .open(file_path)
                .await?;

            let bytes_to_read = if file.size - file_begin > bytes_left  {
                bytes_left
            } else {
                file.size - file_begin
            };
            
            fd.seek(std::io::SeekFrom::Start(file_begin)).await?;

            let mut buffer = vec![0u8; bytes_to_read as usize];

            fd.read_exact(&mut buffer).await?;

            data.extend(buffer);

            block_start += bytes_to_read;
            bytes_left -= bytes_to_read;
            file_index += 1;
        }

        block.data = Some(data);

        Ok(block)
    }

    async fn run(mut self) -> Result<()> {
        let mut writer_handles = Vec::new();
        let mut reader_handles = Vec::new();

        loop {
            let _ = tokio::select! {
                Some(message) = self.rx.recv() => {
                    match message {
                        ClientMessage::Shutdown => {
                            tracing::info!("Shutting down disk writer");
                            break;
                        },
                        ClientMessage::DownloadedBlock{ block } => {
                            if *self.downloaded_pieces_count.lock().await == self.torrent_context.torrent_info.pieces_count {
                                continue;
                            }

                            let torrent_context = Arc::clone(&self.torrent_context);
                            let downloaded = Arc::clone(&self.downloaded);
                            let downloaded_pieces_count = Arc::clone(&self.downloaded_pieces_count);

                            let handle = tokio::spawn(async move {
                                let index = block.index;

                                tracing::debug!("writing block index '{}', piece index '{}', begin '{}', size '{}'", block.number, block.index, block.begin, block.length);
                                if let Err(e) = DiskManager::write_to_file(&torrent_context, block).await {
                                    tracing::error!("Disk writer error: {:?}", e);
                                    return;
                                }
                                tracing::trace!("finished writing block");

                                {
                                    let mut downloaded_guard = downloaded.lock().await;
                                    if let Some(piece) = downloaded_guard.iter_mut().find(|piece| piece.index == index) {
                                        piece.block_count += 1;

                                        if piece.block_count == torrent_context.torrent_info.get_specific_piece_block_count(piece.index) {
                                            if let Err(e) = torrent_context.tx.send(ClientMessage::Have { piece: piece.index }).await {
                                                tracing::error!("Disk writer error: {:?}", e);
                                                return;
                                            }
                                            *downloaded_pieces_count.lock().await += 1;
                                        }
                                    }
                                    else {
                                        let piece = Piece {
                                            index,
                                            block_count: 1,
                                        };

                                        if piece.block_count == torrent_context.torrent_info.get_specific_piece_block_count(piece.index) {
                                            if let Err(e) = torrent_context.tx.send(ClientMessage::Have { piece: piece.index }).await {
                                                tracing::error!("Disk writer error: {:?}", e);
                                                return;
                                            }
                                            *downloaded_pieces_count.lock().await += 1;
                                        }

                                        downloaded_guard.push(piece);
                                    }
                                }
                                
                                if *downloaded_pieces_count.lock().await == torrent_context.torrent_info.pieces_count {
                                    if let Err(e) = torrent_context.tx.send(ClientMessage::FinishedDownloading).await {
                                        tracing::error!("Disk writer error: {:?}", e);
                                        return;
                                    }
                                }
                                
                            });

                            writer_handles.push(handle);
                        },
                        ClientMessage::Request{ block, tx } => {
                            let torrent_context = Arc::clone(&self.torrent_context);

                            let handle = tokio::spawn(async move {
                                println!("seeding piece index '{}', begin '{}' to file", block.index, block.begin);
                                match DiskManager::read_block(&torrent_context, block).await {
                                    Ok(block) => {
                                        if let Err(e) = tx.send(ClientMessage::RequestedBlock{ block }).await {
                                            tracing::error!("Disk reader error: {:?}", e);
                                            return;
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!("Disk reader error: {:?}", e);
                                    }
                                };
                            });

                            reader_handles.push(handle);
                        },
                        _ => {}
                    }
                }
                else => {
                    break;
                }
            };
        }

        for handle in writer_handles {
            handle.await?;
        }

        for handle in reader_handles {
            handle.await?;
        }

        Ok(())
    }
}

