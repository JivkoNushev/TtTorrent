pub mod client;
pub mod torrent;
pub mod tracker;
pub mod peer;
pub mod messager;
pub mod disk_writer;
pub mod utils;

pub const DEBUG_MODE: bool = false;
pub const MAX_CHANNEL_SIZE: usize = 100;
pub const INTERVAL_SECS: u64 = 5;

pub const SOCKET_PATH: &str = "client_state/TtTClient.sock";
pub const STATE_FILE_PATH: &str = "client_state/TtTClient.state";
pub const STATE_TORRENT_FILES_PATH: &str = "client_state/torrent_files";