use std::collections::BTreeMap;

use crate::peer::PeerAddress;
use crate::torrent::torrent_file::{ BencodedValue, Sha1Hash };

pub struct TorrentParser {}

impl TorrentParser {
    pub fn parse_torrent_file(torrent_file: &[u8]) -> BencodedValue {
        parse_torrent_file_(torrent_file)
    } 

    pub fn parse_to_torrent_file(torrent_file: &BencodedValue) -> Vec<u8> {
        parse_to_torrent_file_(torrent_file)
    }

    pub fn parse_tracker_response(torrent_file: &[u8]) -> BencodedValue {
        parse_tracker_response_(torrent_file)
    }
}

fn parse_tracker_response_(torrent_file: &[u8]) -> BencodedValue {
    create_dict(torrent_file, &mut 0)
}

fn parse_torrent_file_(torrent_file: &[u8]) -> BencodedValue {
    let dict = create_dict(torrent_file, &mut 0);
    if dict.torrent_file_is_valid() {
        dict
    }
    else {
        panic!("[Error] Invalid dictionary: doesn't have all the required keys");
    }
}

fn parse_to_torrent_file_(torrent_file: &BencodedValue) -> Vec<u8> {
    to_bencoded_dict(torrent_file)
}

fn to_bencoded_dict(bencoded_dict: &BencodedValue) -> Vec<u8> {
    let mut bencoded_string: Vec<u8> = Vec::new();
    if let BencodedValue::Dict(d) = bencoded_dict {
        bencoded_string.push('d' as u8);
        
        for (key, value) in d {
            // append the len of the key
            let key_len = key.len().to_string();
            let mut key_len = key_len.as_bytes().to_vec();
            bencoded_string.append(&mut key_len);
    
            // append the ':'
            bencoded_string.push(':' as u8);
    
            // append key
            bencoded_string.append(&mut key.clone().as_bytes().to_vec());
    
            match value {
                BencodedValue::ByteSha1Hashes(byte_string) => {
                    // append the len of the word
                    let word_len = byte_string.len() * 20;
                    let word_len = word_len.to_string();
                    let mut word_len = word_len.as_bytes().to_vec();
                    bencoded_string.append(&mut word_len);
    
                    // append the ':'
                    bencoded_string.push(':' as u8);
    
                    for sha1hash in byte_string {
                        bencoded_string.append(&mut sha1hash.0.clone().to_vec());
                    }
                }
                BencodedValue::Integer(integer) => {
                    // append bencoded integer
                    bencoded_string.push('i' as u8);
    
                    let mut word = integer.clone().to_string().as_bytes().to_vec();
                    bencoded_string.append(&mut word);
    
                    bencoded_string.push('e' as u8);
                }
                BencodedValue::List(_list) => {
                    let mut bencoded_l = to_bencoded_list(&value);
    
                    bencoded_string.append(&mut bencoded_l);
                }
                BencodedValue::Dict(_dict) => {
                    let mut bencoded_d = to_bencoded_dict(&value);
    
                    bencoded_string.append(&mut bencoded_d);
                }
                BencodedValue::String(string) => {
                    // append the len of the word
                    let word_len = string.len().to_string();
                    let mut word_len = word_len.as_bytes().to_vec();
                    bencoded_string.append(&mut word_len);
    
                    // append the ':'
                    bencoded_string.push(':' as u8);
    
                    bencoded_string.append(&mut string.clone().as_bytes().to_vec());
    
                }
                _ => panic!("Invalid BencodedValue type")
            }
        }
    }
    else {
        panic!("[Error] Trying to benocode a non dictionary");
    }

    bencoded_string.push('e' as u8);
    bencoded_string
}

fn to_bencoded_list(bencoded_list: &BencodedValue) -> Vec<u8> {
    let mut bencoded_string = Vec::new();
    if let BencodedValue::List(l) = bencoded_list {
        bencoded_string.push('l' as u8);
        for value in l {
            match value {
                BencodedValue::ByteSha1Hashes(byte_string) => {
                    // append the len of the word
                    let word_len = byte_string.len() * 20;
                    let word_len = word_len.to_string();
                    let mut word_len = word_len.as_bytes().to_vec();
                    bencoded_string.append(&mut word_len);

                    // append the ':'
                    bencoded_string.push(':' as u8);

                    for sha1hash in byte_string {
                        bencoded_string.append(&mut sha1hash.0.clone().to_vec());
                    }
                }
                BencodedValue::Integer(integer) => {
                    // append bencoded integer
                    bencoded_string.push('i' as u8);

                    let mut word = integer.clone().to_string().as_bytes().to_vec();
                    bencoded_string.append(&mut word);

                    bencoded_string.push('e' as u8);
                }
                BencodedValue::List(_list) => {
                    let mut bencoded_l = to_bencoded_list(&value);

                    bencoded_string.append(&mut bencoded_l);
                }
                BencodedValue::Dict(_dict) => {
                    let mut bencoded_d = to_bencoded_dict(&value);

                    bencoded_string.append(&mut bencoded_d);
                }
                BencodedValue::String(string) => {
                    // append the len of the word
                    let word_len = string.len().to_string();
                    let mut word_len = word_len.as_bytes().to_vec();
                    bencoded_string.append(&mut word_len);

                    // append the ':'
                    bencoded_string.push(':' as u8);

                    bencoded_string.append(&mut string.clone().as_bytes().to_vec());

                }
                _ => panic!("Invalid BencodedValue type")
            }
        }
    }
    else {
        panic!("[Error] Trying to benocode a non list");
    }

    bencoded_string.push('e' as u8);
    bencoded_string
}

fn parse_bencoded_integer(input: &[u8]) -> i128 {
    let mut index = 0;

    if input.is_empty() {
        panic!("Invalid bencoded integer: empty input");
    }
    
    if input[index] == b'i' {
        index += 1;
    }
    else{
        panic!("Invalid bencoded integer: missing 'i' prefix");
    }

    let mut number = String::new();
    while input[index].is_ascii_digit() {
        number.push(input[index] as char);
        index += 1;
    }

    if number.starts_with('0') && number.len() > 1 {
        panic!("Invalid bencoded integer: leading zeros");
    }

    if input[index] == b'e' {
        if number.is_empty() {
            panic!("Invalid bencoded integer: parsing an empty number");
        }
        number.parse::<i128>().unwrap()
    }
    else if input[index] == b'-' {
        panic!("Invalid bencoded integer: negative integer")
    }
    else {
        panic!("Invalid bencoded integer: missing 'e' suffix")
    }
}

fn parse_integer(input: &[u8]) -> i128 {
    if input.is_empty() {
        panic!("Invalid integer: empty input");
    }
    
    if input[0] == b'-' {
        panic!("Invalid integer: negative integer")
    }

    let mut number = String::new();
    for i in input {
        if !i.is_ascii_digit() {
            break;
        }
        number.push(*i as char);
    }

    if number.is_empty() {
        panic!("Invalid integer: parsing an non-number");
    }

    if number.starts_with('0') && number.len() > 1 {
        panic!("Invalid integer: leading zeros");
    }

    number.parse::<i128>().unwrap()
}

fn create_dict(torrent_file: &[u8], cur_index: &mut usize) -> BencodedValue {
    let torrent_file_len = torrent_file.len();    

    *cur_index += 1; // byte 'd'
    let mut dict = BencodedValue::Dict(BTreeMap::new());

    let mut key = String::new();
    loop {
        if torrent_file_len <= *cur_index {
            panic!("Invalid torrent file: too short")
        }

        if torrent_file[*cur_index] == b'e' {
            break;
        }
        else if torrent_file[*cur_index] == b'd' {
            if key.is_empty() {
                panic!("[Error] Trying to parse a dict with an empty key");
            }

            dict.insert_into_dict(key.clone(), create_dict(torrent_file, cur_index));
            key.clear();
        }
        else if torrent_file[*cur_index] == b'l' {
            if key.is_empty() {
                panic!("[Error] Trying to parse a list with an empty key");
            }

            dict.insert_into_dict(key.clone(), create_list(torrent_file, cur_index));
            key.clear();
        }
        else if torrent_file[*cur_index] == b'i' {
            if key.is_empty() {
                panic!("[Error] Trying to parse an integer with an empty key");
            }

            dict.insert_into_dict(key.clone(), create_int(torrent_file, cur_index));
            key.clear();
        }
        else {
            let word_len = parse_integer(&torrent_file[*cur_index..]) as usize;
            let word_len_chars_len: usize = (word_len.checked_ilog10().unwrap_or(0) + 1) as usize;

            *cur_index += word_len_chars_len + 1; // + 1 for the ':' byte
    
            if key.is_empty() {
                let str_slice = std::str::from_utf8(&torrent_file[*cur_index..*cur_index+word_len]).expect("Couldn't parse a key value from a UTF-8 encoded byte array while parsing a dictionary");
                key = String::from(str_slice);

                *cur_index += word_len;
            }
            else {
                if key == "pieces" {
                    if word_len % 20 != 0 {
                        panic!("[Error] Invalid number of bytes in pieces");
                    }
                    let byte_string = torrent_file[*cur_index..*cur_index + word_len].to_vec();
                    let split_bytes: Vec<Sha1Hash> = byte_string
                        .chunks_exact(20)
                        .map(|chunk| {
                            let mut sha1_chunk: [u8; 20] = [0; 20];
                            sha1_chunk.copy_from_slice(chunk);
                            Sha1Hash(sha1_chunk)
                        })
                        .collect();

                    *cur_index += word_len;

                    dict.insert_into_dict(key.clone(), BencodedValue::ByteSha1Hashes(split_bytes));
                }
                else if key == "peers" {
                    if word_len % 6 != 0 {
                        panic!("[Error] Invalid number of bytes in peers");
                    }

                    let byte_string = torrent_file[*cur_index..*cur_index + word_len].to_vec();
                    let split_bytes: Vec<PeerAddress> = byte_string
                        .chunks_exact(6)
                        .map(|chunk| {
                            let mut ip_port_chunk: [u8; 6] = [0; 6];
                            ip_port_chunk.copy_from_slice(chunk);
                            PeerAddress::new(ip_port_chunk)
                        })
                        .collect();

                    *cur_index += word_len;

                    dict.insert_into_dict(key.clone(), BencodedValue::ByteAddresses(split_bytes));
                }
                else {
                    if key.is_empty() {
                        panic!("[Error] Trying to parse a string or a byte string with an empty key");
                    }

                    let str_slice = std::str::from_utf8(&torrent_file[*cur_index..*cur_index+word_len]).expect("Couldn't parse a word value from a UTF-8 encoded byte array while parsing a dictionary");
                    let word = String::from(str_slice);
    
                    *cur_index += word_len;

                    dict.insert_into_dict(key.clone(), BencodedValue::String(word));
                }
                key.clear();
            }
        }
    }
    *cur_index += 1; // byte 'e'

    dict
}

fn create_list(torrent_file: &[u8], cur_index: &mut usize) -> BencodedValue {
    let torrent_file_len = torrent_file.len();


    *cur_index += 1; // byte 'l'
    let mut list = BencodedValue::List(Vec::new());

    loop {
        if torrent_file_len <= *cur_index {
            panic!("Invalid torrent file: too short")
        }

        if torrent_file[*cur_index] == b'e' {
            break;
        }
        else if torrent_file[*cur_index] == b'd' {
            list.insert_into_list(create_dict(torrent_file, cur_index));
        }
        else if torrent_file[*cur_index] == b'l' {
            list.insert_into_list(create_list(torrent_file, cur_index));
        }
        else if torrent_file[*cur_index] == b'i' {
            list.insert_into_list(create_int(torrent_file, cur_index));
        }
        else {
            let word_len = parse_integer(&torrent_file[*cur_index..]) as usize;
            let word_len_chars_len: usize = (word_len.checked_ilog10().unwrap_or(0) + 1) as usize;

            *cur_index += word_len_chars_len + 1; // + 1 for the ':' byte
            
            let word_str_slice = std::str::from_utf8(&torrent_file[*cur_index..*cur_index + word_len]).expect("Couldn't parse a word value from a UTF-8 encoded byte array while parsing a list");
            let word = String::from(word_str_slice);
            
            *cur_index += word_len;

            list.insert_into_list(BencodedValue::String(word));
        }
    }
    *cur_index += 1; // byte 'e'



    list
}

fn create_int(torrent_file: &[u8], cur_index: &mut usize) -> BencodedValue {
    let num = parse_bencoded_integer(&torrent_file[*cur_index..]);
    
    let len_of_the_num: usize = (num.checked_ilog10().unwrap_or(0) + 1) as usize;
    *cur_index += len_of_the_num + 2; // + 2 for the 'i' and 'e' bytes

    BencodedValue::Integer(num)
}

mod tests {
    use super::*;
    
    #[test]
    fn test_parse_integer() {
        let input = b"123";
        let result = parse_integer(input);
        assert_eq!(result, 123);

        let input = b"123456789123456789123456789123456789123";
        let result = parse_integer(input);
        assert_eq!(result, 123456789123456789123456789123456789123);

        let input = b"0";
        let result = parse_integer(input);
        assert_eq!(result, 0);
    }

    #[test]
    #[should_panic(expected = "Invalid integer: negative integer")]
    fn test_parse_integer_negative_int() {
        let input = b"-123";
        let _result = parse_integer(input);
    }

    #[test]
    #[should_panic(expected = "Invalid integer: empty input")]
    fn test_parse_integer_empty_input() {
        let input = b"";
        let _result = parse_integer(input);
    }

    #[test]
    #[should_panic(expected = "Invalid integer: leading zeros")]
    fn test_parse_integer_leading_zeros() {
        let input = b"0123";
        let _result = parse_integer(input);
    }

    #[test]
    #[should_panic(expected = "Invalid integer: parsing an non-number")]
    fn test_parse_integer_non_number() {
        let input = b"abc";
        let _result = parse_integer(input);
    }

    #[test]
    fn test_parse_bencoded_integer() {
        let input = b"i123e";
        let result = parse_bencoded_integer(input);
        assert_eq!(result, 123);

        let input = b"i123456789123456789123456789123456789123e";
        let result = parse_bencoded_integer(input);
        assert_eq!(result, 123456789123456789123456789123456789123);

        let input = b"i0e";
        let result = parse_bencoded_integer(input);
        assert_eq!(result, 0);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: negative integer")]
    fn test_parse_bencoded_integer_negative_int() {
        let input = b"i-123e";
        let _result = parse_bencoded_integer(input);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: missing 'i' prefix")]
    fn test_parse_bencoded_integer_missing_i() {
        let input = b"123e";
        let _result = parse_bencoded_integer(input);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: missing 'e' suffix")]
    fn test_parse_bencoded_integer_missing_e() {
        let input = b"i123d";
        let _result = parse_bencoded_integer(input);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: parsing an empty number")]
    fn test_parse_bencoded_integer_empty_number() {
        let input = b"ie";
        let _result = parse_bencoded_integer(input);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: leading zeros")]
    fn test_parse_bencoded_integer_leading_zeros() {
        let input = b"i0123e";
        let _result = parse_bencoded_integer(input);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: empty input")]
    fn test_parse_bencoded_integer_empty_input() {
        let input = b"";
        let _result = parse_bencoded_integer(input);
    }

    #[test]
    fn test_create_int() {
        let input = b"i123e";
        let mut cur_index = 0;
        let result = create_int(input, &mut cur_index);
        assert_eq!(result, BencodedValue::Integer(123));
        assert_eq!(cur_index, 5);

        let input = b"i123456789123456789123456789123456789123e";
        let mut cur_index = 0;
        let result = create_int(input, &mut cur_index);
        assert_eq!(result, BencodedValue::Integer(123456789123456789123456789123456789123));
        assert_eq!(cur_index, 41);

        let input = b"i0e";
        let mut cur_index = 0;
        let result = create_int(input, &mut cur_index);
        assert_eq!(result, BencodedValue::Integer(0));
        assert_eq!(cur_index, 3);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: negative integer")]
    fn test_create_int_negative_int() {
        let input = b"i-123e";
        let _result = create_int(input, &mut 0);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: missing 'i' prefix")]
    fn test_create_int_missing_i() {
        let input = b"123e";
        let _result = create_int(input, &mut 0);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: missing 'e' suffix")]
    fn test_create_int_missing_e() {
        let input = b"i123d";
        let _result = create_int(input, &mut 0);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: parsing an empty number")]
    fn test_create_int_empty_number() {
        let input = b"ie";
        let _result = create_int(input, &mut 0);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: leading zeros")]
    fn test_create_int_leading_zeros() {
        let input = b"i0123e";
        let _result = create_int(input, &mut 0);
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: empty input")]
    fn test_create_int_empty_input() {
        let input = b"";
        let _result = create_int(input, &mut 0);
    }

    #[test]
    fn test_create_dict() {
        let torrent_file = "d8:announce5:url:)4:infod4:name4:name12:piece lengthi262144e6:pieces20:丂丂丂丂丂丂AA6:lengthi89eee".as_bytes();

        let _dict = create_dict(&torrent_file, &mut 0);

        assert!(true);
    }

    #[test]
    #[should_panic(expected = "Invalid torrent file: too short")]
    fn test_create_dict_torrent_file_too_short() {
        let torrent_file = "d8:announce5:url:)3:inti89e".as_bytes();

        let _dict = create_dict(&torrent_file, &mut 0);
    }

    #[test]
    #[should_panic(expected = "[Error] Trying to parse a dict with an empty key")]
    fn test_create_dict_empty_key_dict() {
        let torrent_file = "dd8:announce5:url:)4:infod4:name4:name12:piece lengthi262144e6:pieces20:丂丂丂丂丂丂AA6:lengthi89eee".as_bytes();

        let _dict = create_dict(&torrent_file, &mut 0);
    }


    #[test]
    fn test_create_list() {
        let torrent_file = "l4:spami90elee".as_bytes();

        let list = create_list(&torrent_file, &mut 0);
        let list_valid = BencodedValue::List(vec![BencodedValue::String(String::from("spam")), BencodedValue::Integer(90), BencodedValue::List(Vec::new())]);

        assert_eq!(list_valid, list);
    }

    #[test]
    #[should_panic(expected = "Invalid torrent file: too short")]
    fn test_create_list_torrent_file_too_short() {
        let torrent_file = "l4:spami90ele".as_bytes();

        let list = create_list(&torrent_file, &mut 0);
        let list_valid = BencodedValue::List(vec![BencodedValue::String(String::from("spam")), BencodedValue::Integer(90), BencodedValue::List(Vec::new())]);

        assert_eq!(list_valid, list);
    }

    #[test]
    fn test_to_bencoded_dict() {
        let torrent_file = "d8:announce5:url:)4:infod4:name4:name12:piece lengthi262144e6:pieces20:丂丂丂丂丂丂AA6:lengthi89eee".as_bytes();
        let dict: BencodedValue = parse_torrent_file_(torrent_file);

        let torrent_file = parse_to_torrent_file_(&dict);

        let new_torrent_file = String::from_utf8(torrent_file).unwrap();

        let _new_dict: BencodedValue = parse_torrent_file_(new_torrent_file.as_bytes());

        assert!(true);
    }

    #[test]
    fn test_to_bencoded_list() {
        let torrent_file = "l4:spami90elee".as_bytes();

        let list = create_list(&torrent_file, &mut 0);

        let bencoded_list = to_bencoded_list(&list);

        assert_eq!(bencoded_list, torrent_file);
    }

}