pub async fn save_state(&mut self) {
    let state_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open("src/client/client_state/state.json")
        .await
        .unwrap();

    // write the torrents from torrent downloaders to a list of "downloading"
    let state = match state_file.read_to_string().await {
        Ok(state) => state,
        Err(e) => {
            println!("Failed to read state file: {}", e);
            return;
        }
    };

    let mut state = match serde_json::from_str::<serde_json::Value>(&state) {
        Ok(state) => state,
        Err(e) => {
            println!("Failed to parse state file: {}", e);
            return;
        }
    };

    let torrent_downloaders = TORRENT_DOWNLOADERS.lock().await;

    for torrent_downloader in torrent_downloaders.iter() {
        // write the torrent in "downloading"
        state["downloading"].push(torrent_downloader.torrent.lock().await)
    }
}