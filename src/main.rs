use std::{fs::File, fmt::Error, collections::LinkedList, io::Read, string::String};
use serde_json::Value;

fn bencodedStringToJson(bencoded_string: String) -> Result<String, Error> {

    let mut json_string = String::new();

    let mut keyword = String::new();
    let mut next_word_length = 0;
    let mut next_word_length_str = String::new();
    let mut next_word = String::new();

    let mut list_depth: LinkedList<String> = LinkedList::new();

    let mut in_int = false;
    let mut skip_steps = 0;

    for (n, c) in bencoded_string.chars().enumerate() {
        if skip_steps > 0 {
            skip_steps -= 1;
            continue;
        }

        if next_word_length > 1 {
            next_word.push(c);
            next_word_length -= 1;
            continue;
        }

        if next_word_length == 1 {

            next_word.push(c);
            next_word_length -= 1;

            if next_word == "pieces" {
                skip_steps = 0;
                next_word.push_str("\":");

                for j in bencoded_string[n..].chars() {
                    skip_steps += 1;
                    if j == ':' {
                        break;
                    }
                    next_word_length_str.push(j);
                }

                next_word_length = next_word_length_str.parse::<i32>().unwrap();
                next_word_length_str.clear();

                for j in bencoded_string[n+skip_steps..].as_bytes() {
                    if next_word_length == 0 {
                        break;
                    }
                    skip_steps += 1;
                    next_word_length -= 1;

                    next_word.push(*j as char);
                }

                json_string.push('"');
                json_string.push_str(next_word.as_str());
                next_word.clear();
                continue;
            }

            next_word.push('"');

            if ["announce", "announce-list", "comment", "created by", "creation date", "encoding", "info", "length", "name", "piece length", "private", "files", "path", "url-list"].contains(&&next_word[0..next_word.len()-1]) { // , "md5sum"
                next_word.push(':');
                if !(json_string.ends_with('{') || json_string.ends_with('[')) {
                    json_string.push(',');
                }
            }

            json_string.push('"');
            json_string.push_str(next_word.as_str());
            next_word.clear();
            continue;
        }

        match c {
            'i' => {
                // next_word.clear();
                skip_steps = 0;
                for j in bencoded_string[n+1..].chars() {
                    skip_steps += 1;
                    if j == 'e' {
                        break;
                    }
                    next_word.push(j);
                }

                json_string.push_str(next_word.as_str());
                next_word.clear();
                continue;
            },
            'l' => {
                list_depth.push_back('l'.to_string());

                if json_string.ends_with(']') {
                    json_string.push(',');
                }
                json_string.push('[');

                continue;
            },
            'd' => {
                list_depth.push_back('d'.to_string());

                if json_string.ends_with('}') {
                    json_string.push(',');
                }
                json_string.push('{');

                continue;
            },
            'e' => {
                match list_depth.pop_back() {
                    Some(collection) => {
                        match collection.as_str() {
                            "l" => {
                                json_string.push(']');
                            },
                            "d" => {
                                json_string.push('}');
                            },
                            _ => {
                                panic!("Invalid list depth");
                            }
                        }
                            
                    }
                    None => {
                        panic!("Invalid bencoded string");
                    }
                    
                }

                next_word.clear();
            },
            '1'..='9' => {
                skip_steps = 0;
                for (i,j ) in bencoded_string[n..].chars().enumerate() {
                    if j == ':' {
                        break;
                    }
                    skip_steps += 1;
                    next_word_length_str.push(j);
                }
                next_word_length = next_word_length_str.parse::<i32>().unwrap();
                next_word_length_str.clear();

                continue;
            }
            _ => {
                // pass
            }
        }
    }

    Ok(json_string)
}

fn main() {
    let mut torrent_file = File::open("kamasutra.torrent").unwrap();
    let mut bencoded_string = String::new();
    torrent_file.read_to_string(&mut bencoded_string).unwrap();
    
    
    let torrent_file_json = bencodedStringToJson(bencoded_string).unwrap();
    
    println!("{}", torrent_file_json);


    let torrent_json: Value = serde_json::from_str(&torrent_file_json).unwrap();

    // println!("{}", torrent_json["info"]["name"]);

}