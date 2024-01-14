use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::task::JoinHandle;
use tokio::sync::{mpsc, Semaphore};
use anyhow::{anyhow, Result};

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::peer::PieceBlock;
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
pub struct DiskWriterTorrentContext {
    tx: mpsc::Sender<ClientMessage>,

    dest_path: String,
    torrent_name: String,
    torrent_file: TorrentFile,
}

impl DiskWriterTorrentContext {
    pub fn new(tx: mpsc::Sender<ClientMessage>, dest_path: String, torrent_name: String, torrent_file: TorrentFile) -> Self {
        Self {
            tx,

            dest_path,
            torrent_name,
            torrent_file,
        }
    }
}

struct DiskWriter {
    rx: mpsc::Receiver<ClientMessage>,
    torrent_blocks_count: usize,

    torrent_context: Arc<DiskWriterTorrentContext>,
}

impl DiskWriter {
    pub fn new(rx: mpsc::Receiver<ClientMessage>, torrent_context: DiskWriterTorrentContext, torrent_blocks_count: usize) -> Self {
        Self {
            rx,
            torrent_blocks_count,

            torrent_context: Arc::new(torrent_context),
        }
    }

    fn get_piece_size(torrent_context: &DiskWriterTorrentContext, piece_index: usize) -> Result<usize> {
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

    fn get_files(torrent_context: &DiskWriterTorrentContext) -> Result<Vec<BTreeMap<String, BencodedValue>>> {
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

    async fn get_files_to_download(torrent_context: &DiskWriterTorrentContext) -> Result<Vec<DownloadableFile>> {
        let files_to_download = DiskWriter::get_files(torrent_context)?;

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

    async fn write_to_file(torrent_context: Arc<DiskWriterTorrentContext>, block: PieceBlock) -> Result<()> {
        let files = DiskWriter::get_files_to_download(&torrent_context).await?;

        let mut block_start = (block.piece_index * DiskWriter::get_piece_size(&torrent_context, 0)? + block.offset) as u64;

        let mut file_index= 0;
        let mut file_offset = 0;
        
        let mut bytes_left = block.size as u64;
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
                .open(file_path.clone())
                .await?;

            let bytes_to_write = if file.size - file_offset > bytes_left  {
                bytes_left
            } else {
                file.size - file_offset
            };
            
            fd.seek(std::io::SeekFrom::Start(file_offset)).await?;

            let written_bytes = block.size as u64 - bytes_left;

            fd.write_all(
                &block.data[written_bytes as usize..(written_bytes + bytes_to_write) as usize]
            ).await?;

            block_start += bytes_to_write;
            bytes_left -= bytes_to_write;
            file_index += 1;
        }

        Ok(())
    }

    async fn run(mut self) -> Result<()> {
        let max_threads = 4; // TODO: make this based on the number of cores
        let semaphore = Arc::new(Semaphore::new(max_threads));

        let mut writer_handles = Vec::new();

        loop {
            let _ = tokio::select! {
                Some(message) = self.rx.recv() => {
                    match message {
                        ClientMessage::Shutdown => {
                           // println!("Shutting down disk writer");
                            break;
                        },
                        ClientMessage::DownloadedBlock{ block } => {
                            // TODO: maybe save the pieces indexes somewhere and if the client crashes download them again
                            let semaphore_clone = Arc::clone(&semaphore);
                            let torrent_context = Arc::clone(&self.torrent_context);

                            let handle = tokio::spawn(async move {
                                println!("writing piece index '{}', offset '{}' to file", block.piece_index, block.offset);
                                if let Err(e) = semaphore_clone.acquire().await {
                                    eprintln!("Semaphore Error: {}", e);
                                    return;
                                }
                                if let Err(e) = DiskWriter::write_to_file(torrent_context, block).await {
                                    eprintln!("Disk writer error: {:?}", e);
                                }
                            });

                            writer_handles.push(handle);

                            self.torrent_blocks_count -= 1;
                            if self.torrent_blocks_count == 0 {
                                break;
                            }
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

        // send to torrent FinishedDownloading message
        self.torrent_context.tx.send(ClientMessage::FinishedDownloading).await?;

        Ok(())
    }
}

pub struct DiskWriterHandle {
    tx: mpsc::Sender<ClientMessage>,
    join_handle: JoinHandle<()>,

    torrent_name: String,
}

impl DiskWriterHandle {
    pub fn new(torrent_context: DiskWriterTorrentContext, torrent_blocks_count: usize) -> Self {
        let (tx, rx) = mpsc::channel(100);

        let torrent_name = torrent_context.torrent_name.clone();

        let disk_writer = DiskWriter::new(rx, torrent_context, torrent_blocks_count);
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

    pub async fn downloaded_block(&mut self, block: PieceBlock) -> Result<()> {
        self.tx.send(ClientMessage::DownloadedBlock{ block }).await?;
        Ok(())
    }
}

