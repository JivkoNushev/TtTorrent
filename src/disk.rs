use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::task::JoinHandle;
use tokio::sync::{mpsc, Mutex, Semaphore};
use anyhow::{anyhow, Result};

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::peer::PieceBlock;
use crate::torrent::{self, TorrentInfo};
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
    torrent_blocks_count: Arc<Mutex<Vec<usize>>>,

    torrent_context: Arc<DiskTorrentContext>,
}

impl Disk {
    pub fn new(rx: mpsc::Receiver<ClientMessage>, torrent_context: DiskTorrentContext) -> Self {
        Self {
            rx,
            torrent_blocks_count: Arc::new(Mutex::new(Vec::new())),

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

    fn get_files(torrent_context: &DiskTorrentContext) -> Result<Vec<BTreeMap<String, BencodedValue>>> {
        let torrent_dict = torrent_context.torrent_file.get_bencoded_dict_ref().try_into_dict()?;
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => return Err(anyhow!("Could not get info dict from torrent file ref: {}", torrent_context.torrent_name))
        };
        
        let mut all_files = Vec::new();

        if let Ok(files_dict) = info_dict.get_from_dict("files") {
            let files = files_dict.try_into_list()?;

            for file in files {
                let file = file.try_into_dict()?;
                all_files.push(file.clone());
            }
        }
        else {
            let file_name = info_dict.get_from_dict("name")?;
            let file_name = file_name.try_into_byte_string()?;

            let length = info_dict.get_from_dict("length")?;
            let length = length.try_into_integer()?;

            let mut file_dict = BTreeMap::new();
            let path = BencodedValue::List(vec![BencodedValue::ByteString(file_name.clone())]);
            file_dict.insert("path".to_string(), path);
            file_dict.insert("length".to_string(), BencodedValue::Integer(length.clone()));
            all_files.push(file_dict);
        }

        Ok(all_files)
    }

    async fn get_files_to_download(torrent_context: &DiskTorrentContext) -> Result<Vec<DownloadableFile>> {
        let files_to_download = Disk::get_files(torrent_context)?;

        let mut files_to_download = files_to_download
            .iter()
            .map(|file| {
                let size = match file.get("length") {
                    Some(BencodedValue::Integer(size)) => *size as u64,
                    _ => return Err(anyhow!("Error: couldn't get file size"))
                };

                let path = match file.get("path") {
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

                let _md5sum = match file.get("md5sum") {
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

    async fn write_to_file(torrent_context: &DiskTorrentContext, piece_block: PieceBlock) -> Result<()> {
        let files = Disk::get_files_to_download(&torrent_context).await?;

        let mut block_start = (piece_block.piece_index * Disk::get_piece_size(&torrent_context, 0)? + piece_block.offset) as u64;

        let mut file_index= 0;
        let mut file_offset = 0;
        
        let mut bytes_left = piece_block.size as u64;
        while bytes_left > 0 {
            for (i, file) in files.iter().enumerate() {
                // if this piece is in this file
                if file.start <= block_start && block_start < file.start + file.size {
                    file_index = i;
                    file_offset = block_start - file.start;
    
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


            let bytes_to_write = if file.size - file_offset > bytes_left  {
                bytes_left
            } else {
                file.size - file_offset
            };
            
            fd.seek(std::io::SeekFrom::Start(file_offset)).await?;

            let written_bytes = piece_block.size as u64 - bytes_left;

            fd.write_all(
                &piece_block.data[written_bytes as usize..(written_bytes + bytes_to_write) as usize]
            ).await?;

            block_start += bytes_to_write;
            bytes_left -= bytes_to_write;
            file_index += 1;
        }

        Ok(())
    }

    async fn read_block(torrent_context: &DiskTorrentContext, mut piece_block: PieceBlock) -> Result<PieceBlock> {
        piece_block.data.clear();

        let files = Disk::get_files_to_download(&torrent_context).await?;

        let mut block_start = (piece_block.piece_index * Disk::get_piece_size(&torrent_context, 0)? + piece_block.offset) as u64;

        let mut file_index= 0;
        let mut file_offset = 0;
        
        let mut bytes_left = piece_block.size as u64;
        while bytes_left > 0 {
            for (i, file) in files.iter().enumerate() {
                // if this piece is in this file
                if file.start <= block_start && block_start < file.start + file.size {
                    file_index = i;
                    file_offset = block_start - file.start;
    
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

            let bytes_to_read = if file.size - file_offset > bytes_left  {
                bytes_left
            } else {
                file.size - file_offset
            };
            
            fd.seek(std::io::SeekFrom::Start(file_offset)).await?;

            let mut buffer = vec![0u8; bytes_to_read as usize];

            fd.read_exact(&mut buffer).await?;

            piece_block.data.extend(buffer);

            block_start += bytes_to_read;
            bytes_left -= bytes_to_read;
            file_index += 1;
        }

        Ok(piece_block)
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
                            break;
                        },
                        ClientMessage::DownloadedBlock{ piece_block } => {
                            if self.torrent_blocks_count.lock().await.len() == self.torrent_context.torrent_info.blocks_count {
                                continue;
                            }

                            let writer_semaphore = Arc::clone(&writer_semaphore);
                            let torrent_context = Arc::clone(&self.torrent_context);
                            let torrent_blocks_count = Arc::clone(&self.torrent_blocks_count);

                            let handle = tokio::spawn(async move {
                                if let Err(e) = writer_semaphore.acquire().await {
                                    eprintln!("Semaphore Error: {}", e);
                                    return;
                                }

                                let block_numbers_in_current_piece = (piece_block.piece_index * torrent_context.torrent_info.blocks_in_piece..piece_block.piece_index * torrent_context.torrent_info.blocks_in_piece + torrent_context.torrent_info.get_blocks_in_specific_piece(piece_block.piece_index)).collect::<Vec<usize>>();
                                let piece = piece_block.piece_index as u32;
                                
                                torrent_blocks_count.lock().await.push(piece_block.block_index);
                                println!("writing piece index '{}', offset '{}' to file", piece_block.piece_index, piece_block.offset);
                                
                                if let Err(e) = Disk::write_to_file(&torrent_context, piece_block).await {
                                    eprintln!("Disk writer error: {:?}", e);
                                }
                                
                                let mut has_piece = true;
                                for block in block_numbers_in_current_piece {
                                    if !torrent_blocks_count.lock().await.contains(&block) {
                                        has_piece = false;
                                        break
                                    }
                                };

                                if has_piece {
                                    if let Err(e) = torrent_context.tx.send(ClientMessage::Have { piece }).await {
                                        eprintln!("Disk writer error: {:?}", e);
                                    }
                                }
                                
                                if torrent_blocks_count.lock().await.len() == torrent_context.torrent_info.blocks_count {
                                    if let Err(e) = torrent_context.tx.send(ClientMessage::FinishedDownloading).await {
                                        eprintln!("Disk writer error: {:?}", e);
                                    }
                                }
                            });

                            writer_handles.push(handle);
                        },
                        ClientMessage::Request{ piece_block, tx } => {
                            let reader_semaphore = Arc::clone(&reader_semaphore);
                            let torrent_context = Arc::clone(&self.torrent_context);

                            let handle = tokio::spawn(async move {
                                if let Err(e) = reader_semaphore.acquire().await {
                                    eprintln!("Semaphore Error: {}", e);
                                    return;
                                }
                                
                                println!("seeding piece index '{}', offset '{}' to file", piece_block.piece_index, piece_block.offset);
                                match Disk::read_block(&torrent_context, piece_block).await {
                                    Ok(piece_block) => {
                                        if let Err(e) = tx.send(ClientMessage::RequestedBlock{ piece_block }).await {
                                            eprintln!("Disk reader error: {:?}", e);
                                        }
                                    },
                                    Err(e) => {
                                        eprintln!("Disk reader error: {:?}", e);
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
               // println!("Disk writer error: {:?}", e);
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

    pub async fn write_block(&mut self, piece_block: PieceBlock) -> Result<()> {
        self.tx.send(ClientMessage::DownloadedBlock{ piece_block }).await?;
        Ok(())
    }

    pub async fn read_block(&mut self, piece_block: PieceBlock, tx: mpsc::Sender<ClientMessage>) -> Result<()> {
        self.tx.send(ClientMessage::Request{ piece_block, tx }).await?;
        Ok(())
    }
}

