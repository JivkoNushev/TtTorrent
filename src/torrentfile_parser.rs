fn parse_torrent_file(torrent_file_str: Vec<u8>) -> Result<TorrentFileValue, Error> {

    let mut torrent_file = TorrentFileValue::from_dictionary(HashMap::new());

    let mut next_word_length = 0;
    let mut next_word_length_str = String::new();
    let mut next_word = String::new();
    let mut next_int = 0;
    let mut keyword = String::new();
    let mut skip_steps = 0;

    let mut bytes: Vec<u8> = Vec::new();

    for (i, c) in torrent_file_str.iter().enumerate() {

        if skip_steps > 1 {
            skip_steps -= 1;
            continue;
        }

        if next_word_length > 1 {
            next_word.push(*c as char);
            next_word_length -= 1;
            continue;
        }

        if next_word_length == 1 {
            next_word.push(*c as char);
            next_word_length -= 1;

            if next_word == "pieces" {
                // TODO: Handle byte pieces
            }
            if ["announce", "comment", "created by", "encoding", "info", "length", "name", "piece length", "private", "files", "path", "url-list"].contains(&next_word.as_str()) {
                keyword = next_word.clone();
                next_word.clear();
                
                if "files" == keyword.as_str() {
                    if let Some(torrent_file_dict) = torrent_file.pop_dict() {
                        if let Some(info_dict) = torrent_file_dict.get_mut("info") {

                            info_dict.add_to_dict(keyword.clone(), TorrentFileValue::from_list(Vec::new()));
                        } else {
                            // Handle the case where "info" key is not present in the dictionary.
                            // You can choose to ignore or raise an error depending on your use case.
                            println!("'info' key not found in the dictionary.");
                        }
                    } else {
                        // Handle the case where `torrent_file` is None.
                        println!("Torrent file is None.");
                    }
                }
                
                continue;
            }
            else if "announce" == keyword.as_str() {
                
                if let Some(torrent_file_dict) = torrent_file.pop_dict() {
                    torrent_file_dict.insert(keyword.clone(), TorrentFileValue::from_string(next_word.clone()));
                } else {
                    // Handle the case where `torrent_file` is None.
                    println!("Torrent file is None.");
                }

                keyword.clear();
                next_word.clear();
                continue;
            }
            else if "info" == keyword.as_str() {

                if let Some(torrent_file_dict) = torrent_file.pop_dict() {
                    torrent_file_dict.insert(keyword.clone(), TorrentFileValue::from_dictionary(HashMap::new()));
                }

                keyword.clear();
                next_word.clear();
                continue;
            } 
            else if "length" == keyword.as_str() {
                // length is inside the info dictionary inside a dictionary in the files list
                if let Some(torrent_file_dict) = torrent_file.pop_dict() {
                    if let Some(info_dict) = torrent_file_dict.get_mut("info") {
                        if let Some(files_dict) = info_dict.pop_dict() {
                            if let Some(files_list) = files_dict.get_mut("files") {
                                if let Some(file_dict) = files_list.pop_list() {
                                    // first add a dict and then add the length key
                                    file_dict.push(TorrentFileValue::from_dictionary(HashMap::new()));
                                    file_dict.last_mut().unwrap().add_to_dict(keyword.clone(), TorrentFileValue::from_integer(next_int));
                                }
                            }
                        }   
                    }
                }
            }
            torrent_file = TorrentFileValue::from_string(next_word.clone());
            next_word.clear();
            continue;
        }

    }

    Ok(torrent_file)
}