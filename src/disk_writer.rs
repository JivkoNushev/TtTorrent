use tokio::{io::{AsyncSeekExt, AsyncWriteExt}, sync::mpsc, task::JoinHandle};
use anyhow::{Result, Context};

use std::collections::BTreeMap;

use crate::{messager::ClientMessage, torrent::{BencodedValue, TorrentFile}};


#[derive(Debug)]
pub struct DownloadableFile {
    start: u64,
    size: u64,
    path: String,

    // should be 32 HEX characters
    _md5sum: Option<Vec<u8>>
}

struct DiskWriter {
    rx: mpsc::Receiver<ClientMessage>,

    dest_path: String,
    torrent_name: String,
    torrent_file: TorrentFile,
}

impl DiskWriter {
    pub fn new(rx: mpsc::Receiver<ClientMessage>, dest_path: String, torrent_name: String, torrent_file: TorrentFile) -> Self {
        Self {
            rx,

            dest_path,
            torrent_name,
            torrent_file,
        }
    }

    fn get_piece_size(&self, piece_index: usize) -> usize {
        // TODO: fix ? dont use get functions in here

        let torrent_length = self.torrent_file.get_torrent_length();
        let piece_length = self.torrent_file.get_piece_length();

        let pieces_count = torrent_length.div_ceil(piece_length as u64) as usize;

        if piece_index == pieces_count - 1 {
            (torrent_length % piece_length as u64) as usize
        }
        else {
            piece_length
        }
    }

    fn get_files(&self) -> Vec<BTreeMap<String, BencodedValue>> {
        let torrent_dict = match self.torrent_file.get_bencoded_dict_ref().try_into_dict() {
            Some(torrent_dict) => torrent_dict,
            None => panic!("Could not get torrent dict ref from torrent file: {}", self.torrent_name)
        };
        let info_dict = match torrent_dict.get("info") {
            Some(info_dict) => info_dict,
            None => panic!("Could not get info dict from torrent file ref: {}", self.torrent_name)
        };
        
        let mut all_files = Vec::new();

        if let Some(files_dict) = info_dict.get_from_dict("files") {
            let files = match files_dict {
                BencodedValue::List(files) => files,
                _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };

            for file in files {
                let file = match file {
                    BencodedValue::Dict(file) => file,
                    _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
                };

                all_files.push(file.clone());
            }
        }
        else {
            let file_name = match info_dict.get_from_dict("name") {
                Some(file_name) => file_name,
                None => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };
            let file_name = match file_name {
                BencodedValue::ByteString(file_name) => file_name,
                _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };

            let length = match info_dict.get_from_dict("length") {
                Some(length) => length,
                None => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };
            let length = match length {
                BencodedValue::Integer(length) => length,
                _ => panic!("Could not get file size from info dict ref in torrent file: {}", self.torrent_name)
            };

            let mut file_dict = BTreeMap::new();
            let path = BencodedValue::List(vec![BencodedValue::ByteString(file_name)]);
            file_dict.insert("path".to_string(), path);
            file_dict.insert("length".to_string(), BencodedValue::Integer(length.clone()));
            all_files.push(file_dict);
        }

        all_files
    }

    async fn get_files_to_download(&self) -> Vec<DownloadableFile> {
        let files_to_download = self.get_files();

        let mut files_to_download = files_to_download
            .iter()
            .map(|file| {
                let size = match file.get("length") {
                    Some(BencodedValue::Integer(size)) => *size as u64,
                    _ => panic!("Error: couldn't get file size")
                };

                let path = match file.get("path") {
                    Some(BencodedValue::List(path_list)) => {
                        path_list
                            .iter()
                            .map(|path| {
                                match path {
                                    BencodedValue::ByteString(path) => String::from_utf8(path.to_vec()).unwrap(),
                                    _ => panic!("Error: couldn't get file path")
                                }
                            })
                            .collect::<Vec<String>>()
                    },
                    _ => panic!("Error: couldn't get file path")
                };

                let path = path.join("/");

                let _md5sum = match file.get("md5sum") {
                    Some(BencodedValue::ByteString(md5sum)) => Some(md5sum.clone()),
                    _ => None
                };

                DownloadableFile {
                    start: 0,
                    size,
                    path,
                    _md5sum
                }
            })
            .collect::<Vec<DownloadableFile>>();

        // start of file is the size of the previous file
        for i in 1..files_to_download.len() {
            files_to_download[i].start = files_to_download[i - 1].start + files_to_download[i - 1].size;
        }
        
        files_to_download
    }

    async fn write_to_file(&self, piece_index: usize, piece: Vec<u8>, files: &Vec<DownloadableFile>) {
        let piece_length = self.get_piece_size(piece_index) as u64;
        let mut piece_start_offset = (self.get_piece_size(0) * piece_index) as u64;

        let mut file_index= 0;
        let mut file_offset = 0;
        
        let mut bytes_left = piece_length;
        while bytes_left > 0 {
            for (i, file) in files.iter().enumerate() {
                // if this piece is in this file
                if file.start <= piece_start_offset && piece_start_offset < file.start + file.size {
                    file_index = i;
                    file_offset = piece_start_offset - file.start;
    
                    break;
                }
            }  
            
            let file = &files[file_index];

            let file_path = std::path::PathBuf::from(format!("{}/{}", self.dest_path, file.path));
            
            // create the directories if they don't exist
            let file_dir = file_path.parent().unwrap();
            tokio::fs::create_dir_all(file_dir).await.unwrap();

            let mut fd = tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(file_path.clone())
                .await
                .unwrap();

            let bytes_to_write = if file.size - file_offset > bytes_left  {
                bytes_left
            } else {
                file.size - file_offset
            };
            
            fd.seek(std::io::SeekFrom::Start(file_offset)).await.unwrap();

            let written_bytes = piece_length - bytes_left;

            fd.write_all(
                &piece[written_bytes as usize..(written_bytes + bytes_to_write) as usize]
            ).await.unwrap();

            piece_start_offset += bytes_to_write;
            bytes_left -= bytes_to_write;
            file_index += 1;
        }
    }

    async fn run(mut self) -> Result<()> {
        let files_to_download = self.get_files_to_download().await;

        loop {
            tokio::select! {
                Some(message) = self.rx.recv() => {
                    match message {
                        ClientMessage::Shutdown => {
                            println!("Shutting down disk writer");
                            break;
                        },
                        ClientMessage::DownloadedPiece{piece_index, piece} => {
                            // TODO: use multithreading to write to multiple files at once
                            // using semaphores to limit the number of threads ?

                            self.write_to_file(piece_index, piece, &files_to_download).await;
                        },
                        _ => {}
                    }
                }
            }
        }

        // TODO: wait for all writing tasks to finish
        
        Ok(())
    }
}

pub struct DiskWriterHandle {
    tx: mpsc::Sender<ClientMessage>,
    pub join_handle: JoinHandle<()>,

    torrent_name: String,
}

impl DiskWriterHandle {
    pub fn new(dest_path: String, torrent_name: String, torrent_file: TorrentFile) -> Self {
        let (tx, rx) = mpsc::channel(100);

        let disk_writer = DiskWriter::new(rx, dest_path, torrent_name.clone(), torrent_file);

        let join_handle = tokio::spawn(async move {
            if let Err(e) = disk_writer.run().await {
                println!("Disk writer error: {:?}", e);
            }
        });

        Self {
            tx,
            join_handle,
            torrent_name,
        }
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        let _ = self.tx.send(ClientMessage::Shutdown).await?;

        Ok(())
    }

    pub async fn downloaded_piece(&mut self, piece_index: usize, piece: Vec<u8>) -> Result<()> {
        self.tx.send(ClientMessage::DownloadedPiece{piece_index, piece}).await?;

        Ok(())
    }
}

