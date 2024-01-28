use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::task::JoinHandle;
use tokio::sync::{mpsc, Mutex, Semaphore};
use anyhow::{anyhow, Result};

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::peer::block_picker::Piece;
use crate::peer::Block;
use crate::torrent::TorrentInfo;
use crate::{messager::ClientMessage, torrent::{BencodedValue, TorrentFile}};

#[derive(Debug, Clone)]
struct DownloadableFile {
    start: u64,
    size: u64,
    path: String,

    // should be 32 HEX characters
    _md5sum: Option<Vec<u8>>
}

#[derive(Debug, Clone)]
pub struct DiskTorrentContext {
    tx: mpsc::Sender<ClientMessage>,

    dest_path: String,
    torrent_name: String,
    torrent_file: Arc<TorrentFile>,
    torrent_info: Arc<TorrentInfo>,
}

impl DiskTorrentContext {
    pub fn new(tx: mpsc::Sender<ClientMessage>, dest_path: String, torrent_name: String, torrent_file: Arc<TorrentFile>, torrent_info: Arc<TorrentInfo>) -> Self {
        Self {
            tx,

            dest_path,
            torrent_name,
            torrent_file,
            torrent_info,
        }
    }
}

struct Disk {
    rx: mpsc::Receiver<ClientMessage>,
    
    downloaded: Arc<Mutex<Vec<Piece>>>,
    downloaded_pieces_count: Arc<Mutex<usize>>,
    torrent_context: Arc<DiskTorrentContext>,
}

impl Disk {
    pub fn new(rx: mpsc::Receiver<ClientMessage>, torrent_context: DiskTorrentContext) -> Self {
        Self {
            rx,
            
            downloaded: Arc::new(Mutex::new(Vec::new())),
            downloaded_pieces_count: Arc::new(Mutex::new(0)),
            torrent_context: Arc::new(torrent_context),
        }
    }

    fn get_piece_size(torrent_context: &DiskTorrentContext, piece_index: usize) -> Result<usize> {
        // TODO: fix ? dont use get functions in here

        let torrent_length = torrent_context.torrent_file.get_torrent_length()?;
        let piece_length = torrent_context.torrent_file.get_piece_length()?;

        let pieces_count = torrent_length.div_ceil(piece_length as u64) as usize;

        let piece_size = if piece_index == pieces_count - 1 {
            (torrent_length % piece_length as u64) as usize
        } else {
            piece_length
        };

        Ok(piece_size)
    }

    fn get_files(torrent_context: &DiskTorrentContext) -> Result<Vec<BTreeMap<Vec<u8>, BencodedValue>>> {
        let torrent_dict = torrent_context.torrent_file.get_bencoded_dict_ref().try_into_dict()?;
        let info_dict = match torrent_dict.get(&b"info".to_vec()) {
            Some(info_dict) => info_dict,
            None => return Err(anyhow!("Could not get info dict from torrent file ref: {}", torrent_context.torrent_name))
        };
        
        let mut all_files = Vec::new();

        if let Ok(files_dict) = info_dict.get_from_dict(b"files") {
            let files = files_dict.try_into_list()?;

            for file in files {
                let file = file.try_into_dict()?;
                all_files.push(file.clone());
            }
        }
        else {
            let file_name = info_dict.get_from_dict(b"name")?;
            let file_name = file_name.try_into_byte_string()?;

            let length = info_dict.get_from_dict(b"length")?;
            let length = length.try_into_integer()?;

            let mut file_dict = BTreeMap::new();
            let path = BencodedValue::List(vec![BencodedValue::ByteString(file_name.clone())]);
            file_dict.insert(b"path".to_vec(), path);
            file_dict.insert(b"length".to_vec(), BencodedValue::Integer(length.clone()));
            all_files.push(file_dict);
        }

        Ok(all_files)
    }

    async fn get_files_to_download(torrent_context: &DiskTorrentContext) -> Result<Vec<DownloadableFile>> {
        let files_to_download = Disk::get_files(torrent_context)?;

        let mut files_to_download = files_to_download
            .iter()
            .map(|file| {
                let size = match file.get(&b"length".to_vec()) {
                    Some(BencodedValue::Integer(size)) => *size as u64,
                    _ => return Err(anyhow!("Error: couldn't get file size"))
                };

                let path = match file.get(&b"path".to_vec()) {
                    Some(BencodedValue::List(path_list)) => {
                        path_list
                            .iter()
                            .map(|path| {
                                match path {
                                    BencodedValue::ByteString(path) => {
                                        match String::from_utf8(path.to_vec()) {
                                            Ok(path) => Ok(path),
                                            Err(_) => Err(anyhow!("invalid utf8 in torrent file path"))
                                        }
                                    },
                                    _ => Err(anyhow!("invalid torrent file path"))
                                }
                            })
                            .collect::<Result<Vec<String>>>()?
                    },
                    _ => {
                        return Err(anyhow!("invalid torrent file path"));
                    }
                };

                let path = path.join("/");

                let _md5sum = match file.get(&b"md5sum".to_vec()) {
                    Some(BencodedValue::ByteString(md5sum)) => Some(md5sum.clone()),
                    _ => None
                };

                Ok(DownloadableFile {
                    start: 0,
                    size,
                    path,
                    _md5sum
                })
            })
            .collect::<Result<Vec<DownloadableFile>>>()?;
        
        // start of file is the size of the previous file
        for i in 1..files_to_download.len() {
            files_to_download[i].start = files_to_download[i - 1].start + files_to_download[i - 1].size;
        }
        
        Ok(files_to_download)
    }

    async fn write_to_file(torrent_context: &DiskTorrentContext, block: Block) -> Result<()> {
        let files = Disk::get_files_to_download(&torrent_context).await?;

        let data = match block.data {
            Some(data) => data,
            None => {
                tracing::error!("Trying to write block with no data");
                return Err(anyhow!("Trying to write block with no data"));
            }
        };

        let mut block_start = (block.index * Disk::get_piece_size(&torrent_context, 0)? as u32 + block.begin) as u64;

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

            tokio::fs::create_dir_all(file_dir).await.unwrap(); // file_dir is always a directory
            
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

        let files = Disk::get_files_to_download(&torrent_context).await?;

        let mut block_start = (block.index * Disk::get_piece_size(&torrent_context, 0)? as u32 + block.begin) as u64;

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
        let max_threads = 12; // TODO: make this based on the number of cores
        let writer_semaphore = Arc::new(Semaphore::new(max_threads / 2));
        let reader_semaphore = Arc::new(Semaphore::new(max_threads / 2));

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

                            let writer_semaphore = Arc::clone(&writer_semaphore);

                            let torrent_context = Arc::clone(&self.torrent_context);
                            let downloaded = Arc::clone(&self.downloaded);
                            let downloaded_pieces_count = Arc::clone(&self.downloaded_pieces_count);

                            let handle = tokio::spawn(async move {
                                if let Err(e) = writer_semaphore.acquire().await {
                                    tracing::error!("Semaphore Error: {}", e);
                                    return;
                                }
                                
                                let index = block.index;

                                tracing::debug!("writing block index '{}', piece index '{}', begin '{}', size '{}'", block.number, block.index, block.begin, block.length);
                                if let Err(e) = Disk::write_to_file(&torrent_context, block).await {
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
                            let reader_semaphore = Arc::clone(&reader_semaphore);
                            let torrent_context = Arc::clone(&self.torrent_context);

                            let handle = tokio::spawn(async move {
                                if let Err(e) = reader_semaphore.acquire().await {
                                    tracing::error!("Semaphore Error: {}", e);
                                    return;
                                }
                                
                                println!("seeding piece index '{}', begin '{}' to file", block.index, block.begin);
                                match Disk::read_block(&torrent_context, block).await {
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

pub struct DiskHandle {
    pub tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<()>,

    torrent_name: String,
}

impl DiskHandle {
    pub fn new(torrent_context: DiskTorrentContext) -> Self {
        let (tx, rx) = mpsc::channel(100);

        let torrent_name = torrent_context.torrent_name.clone();

        let disk_writer = Disk::new(rx, torrent_context);
        let join_handle = tokio::spawn(async move {
            if let Err(e) = disk_writer.run().await {
               tracing::error!("Disk writer error: {:?}", e);
            }
        });

        Self {
            tx,
            join_handle,
            torrent_name,
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
        let _ = self.tx.send(ClientMessage::Shutdown).await?;

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

