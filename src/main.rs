use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::str::FromStr;

/*
    announce—the URL of the tracker
    info—this maps to a dictionary whose keys are dependent on whether one or more files are being shared:

    files—a list of dictionaries each corresponding to a file (only when multiple files are being shared). Each dictionary has the following keys:
        length—size of the file in bytes.
        path—a list of strings corresponding to subdirectory names, the last of which is the actual file name
    length—size of the file in bytes (only when one file is being shared though)
    name—suggested filename where the file is to be saved (if one file)/suggested directory name where the files are to be saved (if multiple files)
    piece length—number of bytes per piece. This is commonly 28 KiB = 256 KiB = 262,144 B.
    pieces—a hash list, i.e., a concatenation of each piece's SHA-1 hash. As SHA-1 returns a 160-bit hash, pieces will be a string whose length is a multiple of 20 bytes. If the torrent contains multiple files, the pieces are formed by concatenating the files in the order they appear in the files dictionary (i.e., all pieces in the torrent are the full piece length except for the last piece, which may be shorter).
*/

#[derive(Debug)]
struct BencodedDict {
    files: Vec<BencodedDict>,
    // size of the file in bytes (only when one file is being shared though)
    length: u64,
    // a list of strings corresponding to subdirectory names, the last of which is the actual file name
    path: Vec<String>,
    // suggested filename/directory
    name: String,
    // number of bytes per piece
    piece_length: u64,
    // a hash list, i.e., a concatenation of each piece's SHA-1 hash
    pieces: Vec<u8>,
    // if the torrent is private
    private: bool,
    // if the torrent has multiple files
    has_multiple_files: bool
}

impl BencodedDict {
    fn new() -> BencodedDict {
        BencodedDict {
            files: Vec::new(),
            length: 0,
            path: Vec::new(),
            name: String::new(),
            piece_length: 0,
            pieces: Vec::new(),
            private: false,
            has_multiple_files: false
        }
    }
}

#[derive(Debug)]
struct BitTorrentFile {
    // the URL of the tracker
    announce: String,
    // list of lists of strings (only when multiple trackers are being shared)
    announce_list: Vec<Vec<String>>,
    // maps to a dictionary whose keys are dependent on whether one or more files are being shared
    info: BencodedDict,
    // list of dictionaries each corresponding to a file (only when multiple files are being shared)
    piece_layers: Vec<String>,
    // the comment of the torrent
    comment: String,
    // the creation date of the torrent
    creation_date: u64,
    // the name and version of the program used to create the torrent
    created_by: String
}

impl BitTorrentFile {
    fn new(bencoded_string: &str) -> BitTorrentFile {
        let mut bitTorrentFile = BitTorrentFile {
            announce: String::new(),
            announce_list: Vec::new(),
            info: BencodedDict::new(),
            piece_layers: Vec::new(),
            comment: String::new(),
            creation_date: 0,
            created_by: String::new()
        };

        let mut arg_len = 1;
        let mut curr_word = String::new();
        let mut keyword = String::new();

        let mut in_dicts = 0;
        let mut in_lists = 0;


        for (i, c) in bencoded_string.chars().enumerate() {
            
            if arg_len > 0 {
                arg_len -= 1;
                curr_word.push(c);
                continue;
            }
            // TODO: just check if curr_word is in a list of keywords
            if curr_word == "d" {
                in_dicts += 1;
                curr_word = String::new();
            } 
            if curr_word == "l" {
                in_lists += 1;
                curr_word = String::new();
            } 
            else if curr_word == "e" {
                // in_dicts -= 1;
                curr_word = String::new();
            }
            else if curr_word == "announce" {
                keyword = String::from_str("announce").unwrap();
                curr_word = String::new();
            }
            else if curr_word == "announce-list" {
                keyword = String::from_str("announce-list").unwrap();
                curr_word = String::new();
            }
            else if curr_word == "info" {
                keyword = String::from_str("info").unwrap();
                curr_word = String::new();
            }
            else if curr_word == "files" {
                keyword = String::from_str("files").unwrap();
                curr_word = String::new();
            }
            else if curr_word == "length" {
                keyword = String::from_str("length").unwrap();
                curr_word = String::new();
            }
            else if curr_word == "name" {
                keyword = String::from_str("name").unwrap();
                curr_word = String::new();
            }
            else if curr_word == "piece length" {
                keyword = String::from_str("piece length").unwrap();
                curr_word = String::new();
            }
            else if curr_word == "pieces" {
                keyword = String::from_str("pieces").unwrap();
                curr_word = String::new();
            }
            else if curr_word == "comment" {
                keyword = String::from_str("comment").unwrap(); 
                curr_word = String::new();
            }
            else if curr_word.starts_with("i") {
                curr_word.push(c);
                if curr_word.ends_with("e") {
                    // extract number from curr_word
                    let num = curr_word[1..curr_word.len()-1].parse::<u64>().unwrap();
                    // TODO: maybe use a function
                    if keyword == "length" {
                        if bitTorrentFile.info.has_multiple_files {
                            bitTorrentFile.info.files.last_mut().unwrap().length = num;
                        }
                        else {
                            bitTorrentFile.info.length = num;
                        }
                    }
                    else if keyword == "piece length" {
                        bitTorrentFile.info.piece_length = num;
                    }
                    curr_word = String::new();
                }
                continue;
            }
            else if !curr_word.parse::<u64>().is_ok() {
                // TODO: switch case with keywords

                if keyword == "announce" {
                    bitTorrentFile.announce = curr_word.clone();
                    print!("announce: {}", curr_word)
                }
                else if keyword == "announce-list" {
                    bitTorrentFile.announce_list.push(vec![curr_word.clone()]);
                }
                else if keyword == "info" {
                    // BitTorrentFileV1.info = curr_word;
                }
                else if keyword == "files" {
                    bitTorrentFile.info.has_multiple_files = true;
                }
                else if keyword == "name" {
                    bitTorrentFile.info.name = curr_word.clone();
                }
                else if keyword == "path" {
                    bitTorrentFile.info.files.last_mut().unwrap().path.push(curr_word.clone());
                }
                else if keyword == "comment" {
                    bitTorrentFile.comment = curr_word.clone();
                }
                else if keyword == "created by" {
                    bitTorrentFile.created_by = curr_word.clone();
                }
                else if keyword == "creation date" {
                    // bitTorrentFile.creation_date = curr_word.parse::<u64>().unwrap();
                }
                else if keyword == "length" {
                    
                }
                else if keyword == "piece length" {
                    
                }
                else if keyword == "pieces" {

                }
                else {
                    println!("Unknown keyword: {}", keyword);
                    println!("Current word: {}", curr_word);
                }
                curr_word = String::new();
            }
            // check for the next word length
            if c != ':' {
                curr_word.push(c);
            }
            else {
                // println!("Current word: {}", curr_word);
                arg_len = curr_word.parse::<u64>().unwrap();
                curr_word = String::new();
            }
        }

        bitTorrentFile
    }
}


fn main() {
    // bencoded string parser
    let mut f = File::open("tfile2.torrent").expect("file not found");
    let mut contents = String::new();

    f.read_to_string(&mut contents)
        .expect("something went wrong reading the file");

    let bencoded_string = BitTorrentFile::new(&contents);
    println!("bencoded_string: {:?}", bencoded_string);

}
