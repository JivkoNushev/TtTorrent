use std::collections::HashMap;
use std::io::{Result, Read};

// Import necessary functions from other modules
pub mod parsers;

pub use parsers::parse_torrent_file;

/// Represents a SHA-1 hash as an array of 20 bytes.
pub type Sha1Hash = [u8; 20];

/// Represents a value in the Bencode format.
#[derive(Debug, Clone)]
pub enum BencodedValue {
    /// Represents a Bencoded integer.
    Integer(i32),

    /// Represents a Bencoded byte string as a list of SHA-1 hashes.
    ByteString(Vec<Sha1Hash>),

    /// Represents a Bencoded list of values.
    List(Vec<BencodedValue>),

    /// Represents a Bencoded dictionary (key-value pairs).
    Dict(HashMap<String, BencodedValue>),

    /// Represents a Bencoded string.
    String(String),
}

impl BencodedValue {
    /// Inserts a key-value pair into a Bencoded dictionary.
    ///
    /// # Arguments
    ///
    /// * `key` - A string representing the key to insert.
    /// * `value` - The `BencodedValue` to associate with the key.
    ///
    /// # Example
    ///
    /// ```
    /// use my_project::torrent_file::{BencodedValue, Sha1Hash};
    ///
    /// let mut dict = BencodedValue::Dict(HashMap::new());
    /// dict.insert_into_dict("info".to_string(), BencodedValue::Integer(42));
    /// ```
    pub fn insert_into_dict(&mut self, key: String, value: BencodedValue) {
        if let BencodedValue::Dict(d) = self {
            d.insert(key, value);
        }
    }

    /// Appends a value to a Bencoded list.
    ///
    /// # Arguments
    ///
    /// * `value` - The `BencodedValue` to append to the list.
    ///
    /// # Example
    ///
    /// ```
    /// use my_project::torrent_file::BencodedValue;
    ///
    /// let mut list = BencodedValue::List(vec![]);
    /// list.insert_into_list(BencodedValue::String("item".to_string()));
    /// ```
    pub fn insert_into_list(&mut self, value: BencodedValue) {
        if let BencodedValue::List(l) = self {
            l.push(value);
        }
    }
}

/// Reads the contents of a file specified by the given `path` and returns it as a vector of bytes.
///
/// This function opens the file at the specified `path`, reads its contents into a `Vec<u8>`,
/// and returns the vector containing the file's bytes.
///
/// # Arguments
///
/// * `path` - A string representing the file path to read.
///
/// # Returns
///
/// * `Ok(Vec<u8>)` - If the file is successfully read, it returns a `Result` containing
///   a `Vec<u8>` with the file's bytes.
/// * `Err(std::io::Error)` - If any I/O error occurs while reading the file, it returns an error.
///
/// # Example
///
/// ```rust
/// use std::io::Write;
/// use tempfile::tempdir;
/// use my_project::utils::read_torrent_file_as_bytes;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create a temporary directory and file for testing
///     let temp_dir = tempdir()?;
///     let file_path = temp_dir.path().join("test_file.txt");
///     let mut file = std::fs::File::create(&file_path)?;
///     file.write_all(b"Hello, world!")?;
///
///     // Read the file as bytes
///     let bytes = read_torrent_file_as_bytes(&file_path.to_string_lossy())?;
///
///     assert_eq!(bytes, b"Hello, world!");
///
///     Ok(())
/// }
/// ```
///
/// In this example, the `read_torrent_file_as_bytes` function reads the contents of a file
/// and returns them as a `Vec<u8>`. The function is used to read a temporary file created
/// for testing purposes.
pub fn read_torrent_file_as_bytes(path: &str) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut file = std::fs::File::open(path)?;

    file.read_to_end(&mut buf)?;

    Ok(buf)
}
