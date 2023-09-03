use crate::{torrent_file::{TorrentFile, Info, FileInfo}, utils::read_bytes_as_string};

pub use nom::{
    IResult,
    bytes::complete::{take, take_while1},
    character::complete::{char, digit1},
    combinator::map_res,
};

fn parse_info<'a>(input: &'a[u8], current_index: &mut usize) -> IResult<&'a[u8], Info> {

    let mut info = Info::new();

    *current_index += 1;

    loop {
        if input[*current_index] == 'e' as u8 {
            *current_index += 1;
            break;
        }
        let keyword = parse_string(input, current_index).unwrap().1;

        parse_info_keyword(keyword, input, current_index, &mut info);

    }


    Ok((input, info))
}


fn parse_announce_list<'a>(input: &'a[u8], current_index: &mut usize) -> IResult<&'a[u8], Vec<Vec<String>>> {

    let mut announce_list:Vec<Vec<String>> = Vec::new();
    let mut announce_list_iner:Vec<String> = Vec::new();
    
    let mut in_list = 1;
    *current_index += 1;

    loop {
        if input[*current_index] == 'l' as u8 {
            announce_list_iner = Vec::new();
            in_list += 1;
            *current_index += 1;

        }
        else if input[*current_index] == 'e' as u8 {
            in_list -= 1;
            *current_index += 1;
            if in_list == 0 {
                break;
            }
            announce_list.push(announce_list_iner.clone());
        }
        else {
            let announce = parse_string(input, current_index).unwrap().1;
            announce_list_iner.push(announce);
        }
    }


    Ok((input, announce_list))
}

fn parse_bencoded_integer<'a>(input: &'a[u8], current_index: &mut usize) -> IResult<&'a[u8], usize> {
    char('i')(input[*current_index..].as_ref())?;
    *current_index += 1;

    let (_, integer_bytes) = digit1(input[*current_index..].as_ref())?;
    let integer_str = std::str::from_utf8(integer_bytes).unwrap();
    *current_index += integer_str.len();

    char('e')(input[*current_index..].as_ref())?;
    *current_index += 1;

    let parsed_integer = integer_str.parse::<usize>().unwrap();

    Ok((input, parsed_integer))
}

fn parse_integer(input: &[u8]) -> IResult<&[u8], usize> {
    map_res(digit1, |s: &[u8]| {
        String::from_utf8(s.to_vec())
            .unwrap()
            .parse::<usize>()
    })(input)
}

fn parse_string<'a>(input: &'a[u8], current_index: &mut usize) -> IResult<&'a[u8], String> {
    // Implement parsing for strings here
    let s = read_bytes_as_string(input[*current_index..].as_ref());

    println!("String: {:?}", input[*current_index..].as_ref());
    
    let mut string_length = parse_integer(input[*current_index..].as_ref()).unwrap().1;

    *current_index += string_length.to_string().len() + 1; // move to the start of the key +1 means the colon

    let string = String::from_utf8(input[*current_index..*current_index + string_length].to_vec()).unwrap();

    *current_index += string_length;


    Ok((input, string))
}

fn parse_pieces<'a>(input: &'a[u8], current_index: &mut usize) -> IResult<&'a[u8], Vec<Vec<u8>>> {
    let mut pieces:Vec<Vec<u8>> = Vec::new();
    
    let mut byte_count = parse_integer(input[*current_index..].as_ref()).unwrap().1;

    println!("Byte count: {}", byte_count);

    if byte_count % 20 != 0 {
        // TODO: ERROR HANDLING
        panic!("Error: byte count is not a multiple of 20");
    }

    *current_index += byte_count.to_string().len() + 1; // move to the start of the key +1 means the colon
    println!("Current index: {}", *current_index);
    while byte_count > 0 {
        let piece = input[*current_index..*current_index + 20].to_vec();
        pieces.push(piece);
        *current_index += 20;
        byte_count -= 20;
    }
    println!("Current index: {}", *current_index);
    println!("REst of the input {:?}", &input[*current_index..]);
    println!("input len: {}", input.len());

    let input_str = read_bytes_as_string(input.as_ref());
    println!("input str: {}", input_str);

    Ok((input, pieces))
}

fn parse_list<'a>(input: &'a[u8], current_index: &mut usize) -> IResult<&'a[u8], Vec<String>> {
    let mut list:Vec<String> = Vec::new();
    
    *current_index += 1;

    loop {
        if input[*current_index] == 'e' as u8 {
            *current_index += 1;
            break;
        }
        else {
            let list_item = parse_string(input, current_index).unwrap().1;
            list.push(list_item);
        }
    }

    Ok((input, list))
}

fn parse_files<'a>(input: &'a[u8], current_index: &mut usize) -> IResult<&'a[u8], Vec<FileInfo>> {
    let mut files:Vec<FileInfo> = Vec::new();
    let mut file = FileInfo::new();

    let mut in_list = 1;
    *current_index += 1;

    loop {
        if input[*current_index] == 'd' as u8 {
            file = FileInfo::new();
            in_list += 1;
            *current_index += 1;
        }
        else if input[*current_index] == 'e' as u8 {
            in_list -= 1;
            *current_index += 1;
            if in_list == 0 {
                break;
            }
            files.push(file.clone());
        }
        else {
            let keyword = parse_string(input, current_index).unwrap().1;

            parse_file_info_keyword(keyword, input, current_index, &mut file);
        }

    }

    Ok((input, files))
}

fn parse_file_info_keyword<'a>(key: String, input: &'a[u8], current_index: &mut usize, file: &mut FileInfo) {
    if key == "length" {
        let length = parse_bencoded_integer(input, current_index).unwrap().1;
        file.length = length;
    }
    else if key == "md5sum" {
        let md5sum = parse_string(input, current_index).unwrap().1;
        file.md5sum = Some(md5sum);
    }
    else if key == "path" {
        let path = parse_list(input, current_index).unwrap().1;
        file.path = path;
    }
    else {
        println!("Unknown key: {}", key);
    }
}

fn parse_info_keyword<'a>(key: String, input: &'a[u8], current_index: &mut usize, info: &mut Info) {
    if key == "piece length" {
        let piece_length = parse_bencoded_integer(input, current_index).unwrap().1;
        info.piece_length = piece_length;
    }
    else if key == "pieces" {
        println!("Parsing pieces");
        let pieces = parse_pieces(input, current_index).unwrap().1;
        info.pieces = pieces;
    }
    else if key == "private" {
        let private = parse_bencoded_integer(input, current_index).unwrap().1;
        info.private = Some(private == 1);
    }
    else if key == "name" {
        let name = parse_string(input, current_index).unwrap().1;
        info.name = name;
    }
    else if key == "length" {
        let length = parse_bencoded_integer(input, current_index).unwrap().1;
        info.length = Some(length);
    }
    else if key == "md5sum" {
        let md5sum = parse_string(input, current_index).unwrap().1;
        info.md5sum = Some(md5sum);
    }
    else if key == "files" {
        let files = parse_files(input, current_index).unwrap().1;
        info.files = Some(files);
    }
    else {
        println!("Unknown key: {}", key);
    }

}

fn parse_torrent_file_keyword<'a>(key: String, input: &'a[u8], current_index: &mut usize, torrent_file: &mut TorrentFile) {
    if key == "info" {
        let info = parse_info(input, current_index).unwrap().1;
        torrent_file.info = info;
    }
    else if key == "announce" {
        let announce = parse_string(input, current_index).unwrap().1;
        torrent_file.announce = announce;
    }
    else if key == "announce-list" {
        let announce_list = parse_announce_list(input, current_index).unwrap().1;
        torrent_file.announce_list = Some(announce_list);
    }
    else if key == "creation date" {
        let creation_date = parse_bencoded_integer(input, current_index).unwrap().1;
        torrent_file.creation_date = Some(creation_date);
    }
    else if key == "comment" {
        let comment = parse_string(input, current_index).unwrap().1;
        torrent_file.comment = Some(comment);
    }
    else if key == "created by" {
        let created_by = parse_string(input, current_index).unwrap().1;
        torrent_file.created_by = Some(created_by);
    }
    else if key == "encoding" {
        let encoding = parse_string(input, current_index).unwrap().1;
        torrent_file.encoding = Some(encoding);
    }
    else if key == "url-list" {
        *current_index += 2;
    }
    else {
        println!("Unknown key: {}", key);
    }

}

pub fn parse_torrent_file(input: &[u8]) -> IResult<&[u8], TorrentFile> {
    // Implement parsing for the entire torrent file here

    let mut torrent_file = TorrentFile::new();

    let mut current_index = 1;

    while current_index != input.len() - 1 {
        if input[current_index] == '}' as u8 {
            return Ok((&input[current_index..], torrent_file));
        }

        let keyword = parse_string(input, &mut current_index).unwrap().1;

        parse_torrent_file_keyword(keyword, input, &mut current_index, &mut torrent_file);
    }

    Ok((input, torrent_file))
}

