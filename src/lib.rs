pub mod client;
pub mod torrent;
pub mod tracker;
pub mod peer;
pub mod messager;
pub mod disk;
pub mod utils;

pub const DEBUG_MODE: bool = true;

// TRACE > DEBUG > INFO > WARN > ERROR
// TRACE - не съм го използвал; използва се от други библиотеки, които използвам като reqwest 
// DEBUG - показва ми когато записвам или рекуествам нещо и др.
// INFO - показва ми къде започват и спират функциите

pub const TRACING_LEVEL: tracing::Level = tracing::Level::DEBUG; 
pub const MAX_CHANNEL_SIZE: usize = 100;
pub const BLOCK_SIZE: usize = 1 << 14;
pub const BLOCK_REQUEST_COUNT: usize = 5;
pub const SENDING_TO_UI_INTERVAL_SECS: u64 = 3;
pub const SAVE_STATE_INTERVAL_SECS: u64 = 120;
pub const TRACKER_REGULAR_REQUEST_INTERVAL_SECS: u64 = 120;
pub const CLIENT_KEEP_ALIVE_MESSAGE_INTERVAL_SECS: u64 = 120;

pub const SOCKET_PATH: &str = "client_state/TtTClient.sock";
pub const STATE_FILE_PATH: &str = "client_state/TtTClient.state";
pub const STATE_TORRENT_FILES_PATH: &str = "client_state/torrent_files";
pub const SEEDING_PORT: u16 = 6881;