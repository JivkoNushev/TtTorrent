use tokio::sync::mpsc;
use anyhow::{anyhow, Result};

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::utils::bencode::BencodedValue;
use crate::torrent::{TorrentFile, TorrentInfo};
use crate::messager::ClientMessage;

#[derive(Debug, Clone)]
pub struct DownloadableFile {
    pub start: u64,
    pub size: u64,
    pub path: String,

    // should be 32 HEX characters, but for future implementation
    pub _md5sum: Option<Vec<u8>>
}

#[derive(Debug, Clone)]
pub struct DiskTorrentContext {
    pub tx: mpsc::Sender<ClientMessage>,

    pub dest_path: String,
    pub torrent_name: String,
    pub torrent_file: Arc<TorrentFile>,
    pub torrent_info: Arc<TorrentInfo>,
    pub files: Vec<DownloadableFile>,
}

impl DiskTorrentContext {
    pub fn new(tx: mpsc::Sender<ClientMessage>, dest_path: String, torrent_name: String, torrent_file: Arc<TorrentFile>, torrent_info: Arc<TorrentInfo>) -> Result<DiskTorrentContext> {
        let files = get_files_to_download(&torrent_file)?;
        
        Ok(Self {
            tx,

            dest_path,
            torrent_name,
            torrent_file,
            torrent_info,
            files,
        })
    }
}

fn get_files(torrent_file: &TorrentFile) -> Result<Vec<BTreeMap<Vec<u8>, BencodedValue>>> {
    let info_dict = torrent_file.get_bencoded_dict_ref().get_from_dict(b"info")?;
    
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

fn get_files_to_download(torrent_file: &TorrentFile) -> Result<Vec<DownloadableFile>> {
    let files_to_download = get_files(torrent_file)?;

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
                _ => return Err(anyhow!("invalid torrent file path"))
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