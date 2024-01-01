use anyhow::{anyhow, Result};
use std::collections::BTreeMap;

use crate::peer::PeerAddress;
use crate::torrent::torrent_file::{BencodedValue, Sha1Hash};

pub fn parse_tracker_response_(torrent_file: &[u8]) -> Result<BencodedValue> {
    create_dict(torrent_file, &mut 0)
}

pub fn parse_torrent_file_(torrent_file: &[u8]) -> Result<BencodedValue> {
    let dict = create_dict(torrent_file, &mut 0)?;
    if !dict.torrent_file_is_valid() {
        return Err(anyhow!("[Error] Invalid dictionary: doesn't have all the required keys"));
    }

    Ok(dict)
}

pub fn parse_to_torrent_file_(torrent_file: &BencodedValue) -> Result<Vec<u8>> {
    to_bencoded_dict(torrent_file)
}

pub fn to_bencoded_dict(bencoded_dict: &BencodedValue) -> Result<Vec<u8>> {
    let dict = bencoded_dict.try_into_dict()?;
    
    let mut bencoded_string = Vec::new();
    bencoded_string.push(b'd');

    for (key, value) in dict {
        // append the len of the key
        let mut key_len = key
            .len()
            .to_string()
            .as_bytes()
            .to_vec();

        bencoded_string.append(&mut key_len);

        // append the ':'
        bencoded_string.push(b':');

        // append key
        let mut key = key
            .clone()
            .as_bytes()
            .to_vec();

        bencoded_string.append(&mut key);

        match value {
            BencodedValue::Dict(_dict) => {
                let mut bencoded_d = to_bencoded_dict(&value)?;

                bencoded_string.append(&mut bencoded_d);
            }
            BencodedValue::List(_list) => {
                let mut bencoded_l = to_bencoded_list(&value)?;

                bencoded_string.append(&mut bencoded_l);
            }
            BencodedValue::Integer(integer) => {
                bencoded_string.push(b'i');

                let mut word = integer
                    .clone()
                    .to_string()
                    .as_bytes()
                    .to_vec();

                bencoded_string.append(&mut word);

                bencoded_string.push(b'e');
            }
            BencodedValue::ByteString(bytes) => {
                // append the len of the word
                let mut word_len = bytes
                    .len()
                    .to_string()
                    .as_bytes()
                    .to_vec();

                bencoded_string.append(&mut word_len);

                // append the ':'
                bencoded_string.push(b':');

                bencoded_string.append(&mut bytes.clone());
            }
            BencodedValue::ByteSha1Hashes(byte_string) => {
                // append the len of the word
                let mut word_len = (byte_string.len() * 20) // 20 bytes per sha1hash
                    .to_string()
                    .as_bytes()
                    .to_vec(); 

                bencoded_string.append(&mut word_len);

                // append the ':'
                bencoded_string.push(b':');

                // append the sha1hashes
                for sha1hash in byte_string {
                    let mut sha1hash = sha1hash
                        .0
                        .clone()
                        .to_vec();

                    bencoded_string.append(&mut sha1hash);
                }
            }
            _ => {
                return Err(anyhow!("Invalid BencodedValue type"));
            }
        }
    }

    bencoded_string.push(b'e');
    Ok(bencoded_string)
}

pub fn to_bencoded_list(bencoded_list: &BencodedValue) -> Result<Vec<u8>> {
    let list = bencoded_list.try_into_list()?;
    
    let mut bencoded_string = Vec::new();
    bencoded_string.push(b'l');

    for value in list {
        match value {
            BencodedValue::Dict(_dict) => {
                let mut bencoded_d = to_bencoded_dict(&value)?;

                bencoded_string.append(&mut bencoded_d);
            }
            BencodedValue::List(_list) => {
                let mut bencoded_l = to_bencoded_list(&value)?;

                bencoded_string.append(&mut bencoded_l);
            }
            BencodedValue::Integer(integer) => {
                bencoded_string.push(b'i');

                let mut word = integer
                    .clone()
                    .to_string()
                    .as_bytes()
                    .to_vec();

                bencoded_string.append(&mut word);

                bencoded_string.push(b'e');
            }
            BencodedValue::ByteString(bytes) => {
                // append the len of the word
                let mut word_len = bytes
                    .len()
                    .to_string()
                    .as_bytes()
                    .to_vec();

                bencoded_string.append(&mut word_len);

                // append the ':'
                bencoded_string.push(b':');

                // append the word
                bencoded_string.append(&mut bytes.clone());
            }
            BencodedValue::ByteSha1Hashes(byte_string) => {
                // append the len of the word
                let mut word_len = (byte_string.len() * 20) // 20 bytes per sha1hash
                    .to_string()
                    .as_bytes()
                    .to_vec(); 

                bencoded_string.append(&mut word_len);

                // append the ':'
                bencoded_string.push(b':');

                // append the sha1hashes
                for sha1hash in byte_string {
                    let mut sha1hash = sha1hash
                        .0
                        .clone()
                        .to_vec();

                    bencoded_string.append(&mut sha1hash);
                }
            }
            _ => {
                return Err(anyhow!("Invalid BencodedValue type"));
            }
        }
    }

    bencoded_string.push(b'e');

    Ok(bencoded_string)
}

pub fn parse_bencoded_integer(input: &[u8]) -> Result<i64> {
    let mut index = 0;

    if input.is_empty() {
        return Err(anyhow!("Invalid bencoded integer: empty input"));
    }
    
    if input[index] != b'i' {
        return Err(anyhow!("Invalid bencoded integer: missing 'i' prefix"));
    }
    index += 1;
    
    let mut number = String::new();
    if input[index] == b'-' {
        number.push(b'-' as char);
    }

    while input[index].is_ascii_digit() {
        number.push(input[index] as char);
        index += 1;
    }

    if  number.starts_with('0') && number.len() > 1 {
        return Err(anyhow!("Invalid bencoded integer: leading zeros"));
    }

    if number.starts_with("-0") {
        return Err(anyhow!("Invalid bencoded integer: negative zero"));
    }

    if input[index] != b'e' {
        return Err(anyhow!("Invalid bencoded integer: missing 'e' suffix"));
    }

    if number.is_empty() {
        return Err(anyhow!("Invalid bencoded integer: parsing an empty number"));
    }
    
    let number = number.parse::<i64>()?;

    Ok(number)
}

pub fn parse_integer(input: &[u8]) -> Result<i64> {
    if input.is_empty() {
        return Err(anyhow!("Invalid integer: empty input"));
    }

    let mut number = String::new();
    if input[0] == b'-' {
        number.push(b'-' as char);
    }

    for i in input {
        if !i.is_ascii_digit() {
            break;
        }
        number.push(*i as char);
    }

    if number.is_empty() {
        return Err(anyhow!("Invalid integer: parsing an empty number"));
    }

    if number.starts_with('0') && number.len() > 1 {
        return Err(anyhow!("Invalid integer: leading zeros"));
    }

    if number.starts_with("-0") {
        return Err(anyhow!("Invalid integer: negative zero"));
    }

    let number = number.parse::<i64>()?;

    Ok(number)
}

pub fn create_dict(torrent_file: &[u8], cur_index: &mut usize) -> Result<BencodedValue> {
    let torrent_file_len = torrent_file.len();   

    if torrent_file[*cur_index] != b'd' {
        panic!("Invalid torrent file: missing 'd' prefix")
    }
    *cur_index += 1;

    let mut dict = BTreeMap::new();
    let mut key = String::new();
    loop {
        if torrent_file_len <= *cur_index {
            panic!("Invalid torrent file: too short")
        }

        match torrent_file[*cur_index] {
            b'e' => break,
            b'd' => {
                dict.insert(key.clone(), create_dict(torrent_file, cur_index)?);
                key.clear();
            },
            b'l' => {
                dict.insert(key.clone(), create_list(torrent_file, cur_index)?);
                key.clear();
            },
            b'i' => {
                dict.insert(key.clone(), create_int(torrent_file, cur_index)?);
                key.clear();
            },
            // this should be either a key or a value
            _ => {
                let word_len = parse_integer(&torrent_file[*cur_index..])? as usize;
                let word_len_chars_len = (word_len.checked_ilog10().unwrap_or(0) + 1) as usize;

                *cur_index += word_len_chars_len + 1; // + 1 for the ':' byte
        
                if key.is_empty() {
                    key = String::from_utf8(torrent_file[*cur_index..*cur_index + word_len].to_vec())?;
                }
                else {
                    if key == "pieces" {
                        if word_len % 20 != 0 {
                            return Err(anyhow!("[Error] Invalid number of bytes in pieces"));
                        }

                        let sha1_hashes = torrent_file[*cur_index..*cur_index + word_len]
                            .to_vec()
                            .chunks_exact(20)
                            .map(|chunk| {
                                let mut sha1_chunk: [u8; 20] = [0; 20];
                                sha1_chunk.copy_from_slice(chunk);
                                
                                Sha1Hash(sha1_chunk)
                            })
                            .collect::<Vec<Sha1Hash>>();

                        dict.insert(key.clone(), BencodedValue::ByteSha1Hashes(sha1_hashes));
                    }
                    else if key == "peers" {
                        if word_len % 6 != 0 {
                            return Err(anyhow!("[Error] Invalid number of bytes in peers"));
                        }

                        let peer_addresses = torrent_file[*cur_index..*cur_index + word_len]
                            .to_vec()
                            .chunks_exact(6)
                            .map(|chunk| {
                                let mut ip_port_chunk: [u8; 6] = [0; 6];
                                ip_port_chunk.copy_from_slice(chunk);

                                PeerAddress::new(ip_port_chunk)
                            })
                            .collect::<Vec<PeerAddress>>();

                        dict.insert(key.clone(), BencodedValue::ByteAddresses(peer_addresses));
                    }
                    else {
                        let byte_string = torrent_file[*cur_index..*cur_index + word_len].to_vec();
                        dict.insert(key.clone(), BencodedValue::ByteString(byte_string));
                    }

                    key.clear();
                }

                *cur_index += word_len;
            }  
        }              
    }

    if torrent_file[*cur_index] != b'e' {
        return Err(anyhow!("Invalid torrent file: missing 'e' suffix"));
    }
    *cur_index += 1;

    Ok(BencodedValue::Dict(dict))
}

pub fn create_list(torrent_file: &[u8], cur_index: &mut usize) -> Result<BencodedValue> {
    let torrent_file_len = torrent_file.len();

    if torrent_file[*cur_index] != b'l' {
        return Err(anyhow!("Invalid torrent file: missing 'l' prefix"));
    }
    *cur_index += 1;

    let mut list = Vec::new();
    loop {
        if torrent_file_len <= *cur_index {
            return Err(anyhow!("Invalid torrent file: too short"));
        }

        match torrent_file[*cur_index] {
            b'e' => break,
            b'd' => list.push(create_dict(torrent_file, cur_index)?),
            b'l' => list.push(create_list(torrent_file, cur_index)?),
            b'i' => list.push(create_int(torrent_file, cur_index)?),
            // this shoud be a word
            _ => {
                let word_len = parse_integer(&torrent_file[*cur_index..])? as usize;
                let word_len_chars_len = (word_len.checked_ilog10().unwrap_or(0) + 1) as usize;

                *cur_index += word_len_chars_len + 1; // + 1 for the ':' byte
        
                let word = torrent_file[*cur_index..*cur_index + word_len].to_vec();
                
                list.push(BencodedValue::ByteString(word));

                *cur_index += word_len;
            }  
        }
    }

    if torrent_file[*cur_index] != b'e' {
        return Err(anyhow!("Invalid torrent file: missing 'e' suffix"));
    }
    *cur_index += 1;

    Ok(BencodedValue::List(list))
}

pub fn create_int(torrent_file: &[u8], cur_index: &mut usize) -> Result<BencodedValue> {
    let num = parse_bencoded_integer(&torrent_file[*cur_index..])?;
    
    let len_of_the_num: usize = (num.checked_ilog10().unwrap_or(0) + 1) as usize;
    *cur_index += len_of_the_num + 2; // + 2 for the 'i' and 'e' bytes

    Ok(BencodedValue::Integer(num))
}