use anyhow::{anyhow, Result};
use std::collections::BTreeMap;

use crate::peer::PeerAddress;
use crate::utils::sha1hash::Sha1Hash;

use super::BencodedValue;

pub fn parse_from_bencoded_value(bencoded_value: &BencodedValue) -> Result<Vec<u8>> {
    match bencoded_value {
        BencodedValue::Dict(_) => to_bencoded_dict(bencoded_value),
        BencodedValue::List(_) => to_bencoded_list(bencoded_value),
        BencodedValue::Integer(i) => Ok(("i".to_owned() + &i.to_string() + "e").as_bytes().to_vec()),
        BencodedValue::ByteString(byte_string) => Ok(byte_string.clone()),
        BencodedValue::ByteSha1Hashes(sha1hashes) => Ok(
            sha1hashes.iter()
                .flat_map(|sha1hash| sha1hash.0)
                .collect::<Vec<u8>>()
        ),
        BencodedValue::ByteAddresses(byte_addresses) => Ok(
            byte_addresses.iter()
            .flat_map(|peer_address| peer_address.to_vec())
            .collect::<Vec<u8>>()
        ),
    }
}

pub fn parse_to_bencoded_value(bytes: &[u8]) -> Result<BencodedValue> {
    match bytes[0] {
        b'd' => create_dict(bytes, &mut 0),
        b'l' => create_list(bytes, &mut 0),
        b'i' => create_int(bytes, &mut 0),
        _ => {
            let word_len = parse_integer(bytes)? as usize;
            let word_len_chars_len = (word_len.checked_ilog10().unwrap_or(0) + 1) as usize;

            let word = bytes[word_len_chars_len + 1..word_len_chars_len + 1 + word_len].to_vec();

            Ok(BencodedValue::ByteString(word))
        }
    }
}

pub fn to_bencoded_dict(bencoded_dict: &BencodedValue) -> Result<Vec<u8>> {
    let dict = bencoded_dict.try_into_dict()?;
    
    let mut bencoded_dict = vec![b'd'];

    for (key, value) in dict {
        // append the len of the key
        let mut key_len = key
            .len()
            .to_string()
            .as_bytes()
            .to_vec();

        // key_len:key
        bencoded_dict.append(&mut key_len);
        bencoded_dict.push(b':');
        bencoded_dict.append(&mut key.clone());

        match value {
            BencodedValue::Dict(_dict) => {
                let mut bencoded_d = to_bencoded_dict(value)?;

                bencoded_dict.append(&mut bencoded_d);
            }
            BencodedValue::List(_list) => {
                let mut bencoded_l = to_bencoded_list(value)?;

                bencoded_dict.append(&mut bencoded_l);
            }
            BencodedValue::Integer(integer) => {
                let mut integer = integer
                    .clone()
                    .to_string()
                    .as_bytes()
                    .to_vec();

                // i{integer}e
                bencoded_dict.push(b'i');
                bencoded_dict.append(&mut integer);
                bencoded_dict.push(b'e');
            }
            BencodedValue::ByteString(bytes) => {
                let mut word_len = bytes
                    .len()
                    .to_string()
                    .as_bytes()
                    .to_vec();

                // byte count:bytes
                bencoded_dict.append(&mut word_len);
                bencoded_dict.push(b':');
                bencoded_dict.append(&mut bytes.clone());
            }
            BencodedValue::ByteSha1Hashes(byte_string) => {
                let mut word_len = (byte_string.len() * 20) // 20 bytes per sha1hash
                    .to_string()
                    .as_bytes()
                    .to_vec(); 

                // byte count:concatenated sha1 hashes
                bencoded_dict.append(&mut word_len);
                bencoded_dict.push(b':');

                for sha1hash in byte_string {
                    let mut sha1hash = sha1hash
                        .0
                        .clone()
                        .to_vec();

                        bencoded_dict.append(&mut sha1hash);
                }
            }
            BencodedValue::ByteAddresses(byte_addresses) => {
                let mut word_len = (byte_addresses.len() * 6) // 6 bytes per peer address
                    .to_string()
                    .as_bytes()
                    .to_vec(); 

                // byte count:concatenated byte addresses
                bencoded_dict.append(&mut word_len);
                bencoded_dict.push(b':');

                for peer_address in byte_addresses {
                    let mut peer_address = peer_address
                        .to_vec();

                        bencoded_dict.append(&mut peer_address);
                }
            }
        }
    }

    bencoded_dict.push(b'e');
    Ok(bencoded_dict)
}

pub fn to_bencoded_list(bencoded_list: &BencodedValue) -> Result<Vec<u8>> {
    let list = bencoded_list.try_into_list()?;
    
    let mut bencoded_list = vec![b'l'];
    for value in list {
        match value {
            BencodedValue::Dict(_dict) => {
                let mut bencoded_d = to_bencoded_dict(value)?;

                bencoded_list.append(&mut bencoded_d);
            }
            BencodedValue::List(_list) => {
                let mut bencoded_l = to_bencoded_list(value)?;

                bencoded_list.append(&mut bencoded_l);
            }
            BencodedValue::Integer(integer) => {
                let mut integer = integer
                .clone()
                .to_string()
                .as_bytes()
                .to_vec();
            
                // i{integer}e
                bencoded_list.push(b'i');
                bencoded_list.append(&mut integer);
                bencoded_list.push(b'e');
            }
            BencodedValue::ByteString(bytes) => {
                let mut word_len = bytes
                    .len()
                    .to_string()
                    .as_bytes()
                    .to_vec();

                // byte count:bytes
                bencoded_list.append(&mut word_len);
                bencoded_list.push(b':');
                bencoded_list.append(&mut bytes.clone());
            }
            BencodedValue::ByteSha1Hashes(byte_string) => {
                let mut word_len = (byte_string.len() * 20) // 20 bytes per sha1hash
                    .to_string()
                    .as_bytes()
                    .to_vec(); 

                // byte count:concatenated sha1 hashes
                bencoded_list.append(&mut word_len);
                bencoded_list.push(b':');

                for sha1hash in byte_string {
                    let mut sha1hash = sha1hash
                        .0
                        .clone()
                        .to_vec();

                    bencoded_list.append(&mut sha1hash);
                }
            }
            BencodedValue::ByteAddresses(byte_addresses) => {
                let mut word_len = (byte_addresses.len() * 6) // 6 bytes per peer address
                    .to_string()
                    .as_bytes()
                    .to_vec(); 

                // byte count:concatenated byte addresses
                bencoded_list.append(&mut word_len);
                bencoded_list.push(b':');

                for peer_address in byte_addresses {
                    let mut peer_address = peer_address
                        .to_vec();

                    bencoded_list.append(&mut peer_address);
                }
            }
        }
    }

    bencoded_list.push(b'e');

    Ok(bencoded_list)
}

pub fn parse_bencoded_integer(bytes: &[u8]) -> Result<i64> {
    let mut index = 0;

    if bytes.is_empty() {
        return Err(anyhow!("Invalid bencoded integer: empty input"));
    }
    
    if bytes[index] != b'i' {
        return Err(anyhow!("Invalid bencoded integer: missing 'i' prefix"));
    }
    index += 1;
    
    let mut number = String::new();
    if bytes[index] == b'-' {
        number.push(b'-' as char);
        index += 1;
    }

    while bytes[index].is_ascii_digit() {
        number.push(bytes[index] as char);
        index += 1;
    }

    if  number.starts_with('0') && number.len() > 1 {
        return Err(anyhow!("Invalid bencoded integer: leading zeros"));
    }

    if number.starts_with("-0") {
        return Err(anyhow!("Invalid bencoded integer: negative zero"));
    }

    if bytes[index] != b'e' {
        return Err(anyhow!("Invalid bencoded integer: missing 'e' suffix"));
    }

    if number.is_empty() {
        return Err(anyhow!("Invalid bencoded integer: parsing an empty number"));
    }
    
    let number = number.parse::<i64>()?;

    Ok(number)
}

pub fn parse_integer(bytes: &[u8]) -> Result<i64> {
    if bytes.is_empty() {
        return Err(anyhow!("Invalid integer: empty input"));
    }
    let mut bytes = bytes;

    let mut number = String::new();
    if bytes[0] == b'-' {
        number.push(b'-' as char);
        bytes = &bytes[1..];
    }

    for i in bytes {

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

pub fn create_dict(bytes: &[u8], cur_index: &mut usize) -> Result<BencodedValue> {
    let bytes_count = bytes.len();   

    if bytes[*cur_index] != b'd' {
        return Err(anyhow!("Invalid torrent file: missing 'd' prefix"));
    }
    *cur_index += 1;

    let mut dict = BTreeMap::new();
    let mut key = Vec::new();
    loop {
        if bytes_count <= *cur_index {
            return Err(anyhow!("Invalid torrent file: too short"));
        }

        match bytes[*cur_index] {
            b'e' => break,
            b'd' => {
                dict.insert(key.clone(), create_dict(bytes, cur_index)?);
                key.clear();
            },
            b'l' => {
                dict.insert(key.clone(), create_list(bytes, cur_index)?);
                key.clear();
            },
            b'i' => {
                dict.insert(key.clone(), create_int(bytes, cur_index)?);
                key.clear();
            },
            // this should be either a key or a value
            _ => {
                let word_len = parse_integer(&bytes[*cur_index..])? as usize;
                let word_len_chars_len = (word_len.checked_ilog10().unwrap_or(0) + 1) as usize;

                *cur_index += word_len_chars_len + 1; // + 1 for the ':' byte
        
                if key.is_empty() {
                    key = bytes[*cur_index..*cur_index + word_len].to_vec();
                }
                else {
                    if key == b"pieces" {
                        if word_len % 20 != 0 {
                            return Err(anyhow!("[Error] Invalid number of bytes in pieces"));
                        }

                        let sha1_hashes = bytes[*cur_index..*cur_index + word_len]
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
                    else if key == b"peers" {
                        if word_len % 6 != 0 {
                            return Err(anyhow!("[Error] Invalid number of bytes in peers"));
                        }

                        let peer_addresses = bytes[*cur_index..*cur_index + word_len]
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
                        let byte_string = bytes[*cur_index..*cur_index + word_len].to_vec();
                        dict.insert(key.clone(), BencodedValue::ByteString(byte_string));
                    }

                    key.clear();
                }

                *cur_index += word_len;
            }  
        }              
    }

    if bytes[*cur_index] != b'e' {
        return Err(anyhow!("Invalid torrent file: missing 'e' suffix"));
    }
    *cur_index += 1;

    Ok(BencodedValue::Dict(dict))
}

pub fn create_list(bytes: &[u8], cur_index: &mut usize) -> Result<BencodedValue> {
    let bytes_count = bytes.len();

    if bytes[*cur_index] != b'l' {
        return Err(anyhow!("Invalid torrent file: missing 'l' prefix"));
    }
    *cur_index += 1;

    let mut list = Vec::new();
    loop {
        if bytes_count <= *cur_index {
            return Err(anyhow!("Invalid torrent file: too short"));
        }

        match bytes[*cur_index] {
            b'e' => break,
            b'd' => list.push(create_dict(bytes, cur_index)?),
            b'l' => list.push(create_list(bytes, cur_index)?),
            b'i' => list.push(create_int(bytes, cur_index)?),
            // this should be a word
            _ => {
                let word_len = parse_integer(&bytes[*cur_index..])? as usize;
                let word_len_chars_len = (word_len.checked_ilog10().unwrap_or(0) + 1) as usize;

                *cur_index += word_len_chars_len + 1; // + 1 for the ':' byte
        
                let word = bytes[*cur_index..*cur_index + word_len].to_vec();
                
                list.push(BencodedValue::ByteString(word));

                *cur_index += word_len;
            }  
        }
    }

    if bytes[*cur_index] != b'e' {
        return Err(anyhow!("Invalid torrent file: missing 'e' suffix"));
    }
    *cur_index += 1;

    Ok(BencodedValue::List(list))
}

pub fn create_int(bytes: &[u8], cur_index: &mut usize) -> Result<BencodedValue> {
    let num = parse_bencoded_integer(&bytes[*cur_index..])?;
    
    let len_of_the_num: usize = (num.checked_ilog10().unwrap_or(0) + 1) as usize;
    *cur_index += len_of_the_num + 2; // + 2 for the 'i' and 'e' bytes

    Ok(BencodedValue::Integer(num))
}