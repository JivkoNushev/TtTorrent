pub mod client;
pub mod torrent;
pub mod tracker;
pub mod peer;
pub mod messager;
pub mod disk_manager;
pub mod utils;
pub mod client_options;

use once_cell::sync::Lazy;

pub static mut CLIENT_OPTIONS: Lazy<client_options::ClientOptions> = Lazy::new(client_options::ClientOptions::default);