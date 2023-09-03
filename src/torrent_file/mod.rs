mod parsers;
mod utils;

pub use parsers::parse_torrent_file;
pub use utils::read_torrent_file_as_bytes;


#[derive(Debug)]
pub struct TorrentFile {
    pub info: Info, // a dictionary that describes the file(s) of the torrent (info dictionary)
    pub announce: String, // The announce URL of the tracker (string)
    pub announce_list: Option<Vec<Vec<String>>>, // this is an extention to the official specification, offering backwards-compatibility. (list of lists of strings).
    pub creation_date: Option<usize>, // the creation time of the torrent, in standard UNIX epoch format (integer, seconds since 1-Jan-1970 00:00:00 UTC)
    pub comment: Option<String>, // free-form textual comments of the author (string)
    pub created_by: Option<String>, // name and version of the program used to create the .torrent (string)
    pub encoding: Option<String>, // the string encoding format used to generate the pieces part of the info dictionary in the .torrent metafile (string)
} 

impl TorrentFile {
    pub fn new() -> TorrentFile {
        TorrentFile {
            info: Info::new(),
            announce: String::new(),
            announce_list: None,
            creation_date: None,
            comment: None,
            created_by: None,
            encoding: None,
        }
    }
}

// Define the Info struct that represents the "info" dictionary
#[derive(Debug)]
pub struct Info {
    pub piece_length: usize, // Integer
    pub pieces: Vec<Vec<u8>>, // ByteStrings of length 20
    pub private: Option<bool>, // Boolean
    pub name: String, // String
    pub length: Option<usize>, // Integer
    pub md5sum: Option<String>, // String
    pub files: Option<Vec<FileInfo>>, // List of FileInfo structs
}

impl Info {
    fn new() -> Info {
        Info {
            name: String::new(),
            piece_length: 0,
            pieces: Vec::new(),
            length: None,
            private: None,
            md5sum: None,
            files: None,
        }
    }
}

// Define a struct to represent file information within the "info" dictionary
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub length: usize, // Integer
    pub md5sum: Option<String>, // String
    pub path: Vec<String>, // List of strings that needs to be separated by "/"
}

impl FileInfo {
    fn new() -> FileInfo {
        FileInfo {
            length: 0,
            md5sum: None,
            path: Vec::new(),
        }
    }
}
