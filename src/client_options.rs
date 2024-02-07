const DEBUG_MODE: bool = false;
const TRACING_LEVEL: tracing::Level = tracing::Level::INFO; 
const MAX_CHANNEL_SIZE: usize = 100;
const BLOCK_SIZE: usize = 1 << 14;
const BLOCK_REQUEST_COUNT: usize = 5;
const SENDING_TO_UI_INTERVAL_SECS: u64 = 3;
const SAVE_STATE_INTERVAL_SECS: u64 = 120;
const TRACKER_REGULAR_REQUEST_INTERVAL_SECS: u64 = 120;
const CLIENT_KEEP_ALIVE_MESSAGE_INTERVAL_SECS: u64 = 120;
const LISTENING_PORT: u16 = 6881;

pub struct ClientOptions {
    pub debug_mode: bool,
    pub tracing_level: tracing::Level,
    pub max_channel_size: usize,
    pub block_size: usize,
    pub block_request_count: usize,
    pub sending_to_ui_interval_secs: u64,
    pub save_state_interval_secs: u64,
    pub tracker_regular_request_interval_secs: u64,
    pub client_keep_alive_message_interval_secs: u64,
    pub socket_path: String,
    pub state_file_path: String,
    pub state_torrent_files_path: String,
    pub listening_port: u16,
}

impl ClientOptions {
    pub fn default() -> ClientOptions {
        ClientOptions {
            debug_mode: DEBUG_MODE,
            tracing_level: TRACING_LEVEL,
            max_channel_size: MAX_CHANNEL_SIZE,
            block_size: BLOCK_SIZE,
            block_request_count: BLOCK_REQUEST_COUNT,
            sending_to_ui_interval_secs: SENDING_TO_UI_INTERVAL_SECS,
            save_state_interval_secs: SAVE_STATE_INTERVAL_SECS,
            tracker_regular_request_interval_secs: TRACKER_REGULAR_REQUEST_INTERVAL_SECS,
            client_keep_alive_message_interval_secs: CLIENT_KEEP_ALIVE_MESSAGE_INTERVAL_SECS,
            socket_path: "client_state/TtTClient.sock".to_string(),
            state_file_path: "client_state/TtTClient.state".to_string(),
            state_torrent_files_path: "client_state/torrent_files".to_string(),
            listening_port: LISTENING_PORT,
        }
    }
}

pub fn setup_options(args: std::env::Args) {
    let argv: Vec<String> = args.collect();

    if argv.contains(&String::from("--help")) {
        print_help_menu();
        std::process::exit(0);
    }

    let mut argv_iter = argv.iter();
    argv_iter.next(); // skip the first argument which is the program name
    while let Some(arg) = argv_iter.next() {
        if arg == "--debug" {
            unsafe { crate::CLIENT_OPTIONS.debug_mode = true; }
        }
        else if arg == "--tracing-level" {
            if let Some(arg) = argv_iter.next() {
                match arg.as_str() {
                    "trace" => unsafe { crate::CLIENT_OPTIONS.tracing_level = tracing::Level::TRACE; },
                    "debug" => unsafe { crate::CLIENT_OPTIONS.tracing_level = tracing::Level::DEBUG; },
                    "info" => unsafe { crate::CLIENT_OPTIONS.tracing_level = tracing::Level::INFO; },
                    "warn" => unsafe { crate::CLIENT_OPTIONS.tracing_level = tracing::Level::WARN; },
                    "error" => unsafe { crate::CLIENT_OPTIONS.tracing_level = tracing::Level::ERROR; },
                    _ => {
                        print_error_menu();
                        std::process::exit(1);
                    }
                }
            }
            else {
                print_error_menu();
                std::process::exit(1); 
            }
        }
        else if arg == "--max-channel-size" {
            if let Some(arg) = argv_iter.next() {
                if let Ok(size) = arg.parse::<usize>() {
                    unsafe { crate::CLIENT_OPTIONS.max_channel_size = size; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else if arg == "--block-size" {
            if let Some(arg) = argv_iter.next() {
                if let Ok(size) = arg.parse::<usize>() {
                    unsafe { crate::CLIENT_OPTIONS.block_size = size; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else if arg == "--block-request-count" {
            if let Some(arg) = argv_iter.next() {
                if let Ok(count) = arg.parse::<usize>() {
                    unsafe { crate::CLIENT_OPTIONS.block_request_count = count; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else if arg == "--sending-to-ui-interval-secs" {
            if let Some(arg) = argv_iter.next() {
                if let Ok(secs) = arg.parse::<u64>() {
                    unsafe { crate::CLIENT_OPTIONS.sending_to_ui_interval_secs = secs; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else if arg == "--save-state-interval-secs" {
            if let Some(arg) = argv_iter.next() {
                if let Ok(secs) = arg.parse::<u64>() {
                    unsafe { crate::CLIENT_OPTIONS.save_state_interval_secs = secs; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else if arg == "--tracker-regular-request-interval-secs" {
            if let Some(arg) = argv_iter.next() {
                if let Ok(secs) = arg.parse::<u64>() {
                    unsafe { crate::CLIENT_OPTIONS.tracker_regular_request_interval_secs = secs; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else if arg == "--client-keep-alive-message-interval-secs" {
            if let Some(arg) = argv_iter.next() {
                if let Ok(secs) = arg.parse::<u64>() {
                    unsafe { crate::CLIENT_OPTIONS.client_keep_alive_message_interval_secs = secs; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else if arg == "--socket-path" {
            if let Some(arg) = argv_iter.next() {
                let path = std::path::Path::new(arg);
                if std::fs::create_dir_all(path).is_ok() {
                    let path = path.join("TtTClient.sock");
                    let path = path.to_str().unwrap().to_string();
                    unsafe { crate::CLIENT_OPTIONS.socket_path = path; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else if arg == "--state-file-path" {
            if let Some(arg) = argv_iter.next() {
                let path = std::path::Path::new(arg);
                if std::fs::create_dir_all(path).is_ok() {
                    let path = path.join("TtTClient.state");
                    let path = path.to_str().unwrap().to_string();
                    unsafe { crate::CLIENT_OPTIONS.state_file_path = path; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else if arg == "--state-torrent-files-path" {
            if let Some(arg) = argv_iter.next() {
                let path = std::path::Path::new(arg);
                if std::fs::create_dir_all(path).is_ok() {
                    let path = path.join("torrent_files");
                    let path = path.to_str().unwrap().to_string();
                    unsafe { crate::CLIENT_OPTIONS.state_torrent_files_path = path; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else if arg == "--listening-port" {
            if let Some(arg) = argv_iter.next() {
                if let Ok(port) = arg.parse::<u16>() {
                    unsafe { crate::CLIENT_OPTIONS.listening_port = port; }
                }
                else {
                    print_error_menu();
                    std::process::exit(1);
                }
            }
            else {
                print_error_menu();
                std::process::exit(1);
            }
        }
        else {
            print_error_menu();
            std::process::exit(1);
        }
    }   
}

fn print_error_menu() {
    println!("Invalid arguments. Use --help for help menu");
}

fn print_help_menu() {
    println!("Usage: tttorrent-client [options]");
    println!("Options:");
    println!("  --help  -  print this help message and exit");
    println!("  --debug  -  set tracing level to DEBUG");
    println!("  --trace  -  set tracing level to TRACE");
}