use nom::{
    IResult,
    bytes::complete::{tag, take_while_m_n},

};

use std::collections::HashMap;

use super::{BencodedValue, Sha1Hash};

/// Parses a Bencoded torrent file from a byte slice and returns a BencodedValue.
///
/// This function takes a byte slice `input` containing a Bencoded torrent file and
/// returns a BencodedValue representing the parsed data structure.
///
/// # Arguments
///
/// * `input` - A reference to a byte slice containing the Bencoded torrent file to be parsed.
///
/// # Returns
///
/// A `BencodedValue` representing the parsed data structure.
///
/// # Example
///
/// ```rust
/// let torrent_bytes = b"d6:lengthi123e4:name5:fileaee";
/// let result = parse_torrent_file(torrent_bytes);
/// ```
///
/// In this example, the `parse_torrent_file` function parses a Bencoded torrent file
/// and returns a `BencodedValue` representing the parsed data.
pub fn parse_torrent_file(input: &[u8]) -> BencodedValue {
    create_dict(input, &mut 0)
}


fn parse_bencoded_integer(input: &[u8]) -> IResult<&[u8], (i32, usize)> {
    let (remaining, number_bytes) = nom::sequence::preceded(
        tag(b"i"), // Parse the "i" prefix
        nom::sequence::terminated(
            take_while_m_n(1, 10, |c| c >= b'0' && c <= b'9'), // Parse a numeric string
            tag(b"e"), // Parse the "e" suffix
        ),
    )(input)?;

    let length = number_bytes.len();
    let number = std::str::from_utf8(number_bytes)
        .unwrap()
        .parse::<i32>()
        .unwrap();

    Ok((remaining, (number, length)))
}


fn parse_integer(input: &[u8]) -> IResult<&[u8], (i32, usize)> {
    // Parse a number with length from 1 to 10
    let (remaining, number_bytes) = take_while_m_n(1, 10, |c| c >= b'0' && c <= b'9')(input)?;
    
    let length = number_bytes.len();
    let number = std::str::from_utf8(number_bytes)
        .unwrap()
        .parse::<i32>()
        .unwrap();

    Ok((remaining, (number, length)))
}

fn create_dict(torrent_file: &[u8], cur_index: &mut usize) -> BencodedValue {
    
    *cur_index += 1; // byte 'd'
    let mut dict = BencodedValue::Dict(HashMap::new());

    let mut key = String::new();
    while torrent_file[*cur_index] != b'e' {

        if torrent_file[*cur_index] == b'd' {
            dict.insert_into_dict(key.clone(), create_dict(torrent_file, cur_index));
            key.clear();
        }
        else if torrent_file[*cur_index] == b'l' {
            dict.insert_into_dict(key.clone(), create_list(torrent_file, cur_index));
            key.clear();
        }
        else if torrent_file[*cur_index] == b'i' {
            dict.insert_into_dict(key.clone(), create_int(torrent_file, cur_index));
            key.clear();
        }
        else {
            let mut word_len: usize; 
            match parse_integer(&torrent_file[*cur_index..]) {
                Ok((_, (num, num_len))) => {
                    *cur_index += num_len;
                    word_len = num as usize;
                },
                Err(e) => panic!("[Error] Can't parse integer at index {cur_index} with error message: {e}\nTrying to parse a word for dictionary")
            }
    
            *cur_index += 1; // byte ':'
    
            if key.is_empty() {
                let str_slice = std::str::from_utf8(&torrent_file[*cur_index..*cur_index+word_len]).expect("Couldn't parse a key value from a UTF-8 encoded byte array while parsing a dictionary");
                key = String::from(str_slice);
                *cur_index += word_len;
            }
            else {
                let word: String; 
    
                if key == "pieces" {
                    if word_len % 20 != 0 {
                        panic!("[Error] Invalid number of bytes in pieces");
                    }

                    let byte_string = torrent_file[*cur_index..*cur_index+word_len].to_vec();

                    let split_bytes: Vec<Sha1Hash> = byte_string
                        .chunks_exact(20)
                        .map(|chunk| {
                            let mut sha1_chunk: Sha1Hash = [0; 20];
                            sha1_chunk.copy_from_slice(chunk);
                            sha1_chunk
                        })
                        .collect();

                    dict.insert_into_dict(key.clone(), BencodedValue::ByteString(split_bytes));
                    *cur_index += word_len;

                }
                else {
                    let str_slice = std::str::from_utf8(&torrent_file[*cur_index..*cur_index+word_len]).expect("Couldn't parse a word value from a UTF-8 encoded byte array while parsing a dictionary");
                    word = String::from(str_slice);
    
                    dict.insert_into_dict(key.clone(), BencodedValue::String(word));
                    
                    *cur_index += word_len;
                }
                key.clear();
            }
        }
    }
    *cur_index += 1; // byte 'e'

    dict
}

fn create_list(torrent_file: &[u8], cur_index: &mut usize) -> BencodedValue {
    
    *cur_index += 1; // byte 'l'
    let mut list = BencodedValue::List(Vec::new());

    while torrent_file[*cur_index] != b'e' {

        if torrent_file[*cur_index] == b'd' {
            list.insert_into_list(create_dict(torrent_file, cur_index));
        }
        else if torrent_file[*cur_index] == b'l' {
            list.insert_into_list(create_list(torrent_file, cur_index));
        }
        else if torrent_file[*cur_index] == b'i' {
            list.insert_into_list(create_int(torrent_file, cur_index));
        }
        else {
            let mut word: String; 
            let mut word_len: usize; 
            
            match parse_integer(&torrent_file[*cur_index..]) {
                Ok((_, (num, num_len))) => {
                    *cur_index += num_len;
                    word_len = num as usize;
                },
                Err(e) => panic!("[Error] Can't parse integer at index {cur_index} with error message: {e}\nTrying to parse a word for list")
            }
    
            *cur_index += 1; // byte ':'
            
    
            let str_slice = std::str::from_utf8(&torrent_file[*cur_index..*cur_index+word_len]).expect("Couldn't parse a word value from a UTF-8 encoded byte array while parsing a list");
            word = String::from(str_slice);
    
            list.insert_into_list(BencodedValue::String(word));
            
            *cur_index += word_len;
        }
    }
    *cur_index += 1; // byte 'e'

    list
}

fn create_int(torrent_file: &[u8], cur_index: &mut usize) -> BencodedValue {

    match parse_bencoded_integer(&torrent_file[*cur_index..]) {
        Ok((_, (num, num_len))) => {
            *cur_index += num_len + 2; // including the 'i' and 'e' bytes
            return BencodedValue::Integer(num);
        },
        Err(e) => eprintln!("Error parsing bencoded value: {e}\nUsed 0 for integer")
    }

    BencodedValue::Integer(0)
}
