use crate::torrent_file::{TorrentFile, Info};

pub use nom::{
    IResult,
    bytes::complete::{take, take_while},
    character::complete::{char, digit1}
};

fn parse_integer(input: &[u8]) -> IResult<&[u8], i64> {
    // Implement parsing for integers here
    Ok((input, 0))
}

fn parse_string(input: &[u8]) -> IResult<&[u8], String> {
    // Implement parsing for strings here
    Ok((input, String::new()))
}

fn parse_info(input: &[u8]) -> IResult<&[u8], Info> {
    // Implement parsing for the info section here
    Ok((input, Info {}))
}

fn parse_announce(input: &[u8]) -> IResult<&[u8], String> {
    // Implement parsing for the announce section here
    Ok((input, String::new()))
}

pub fn parse_torrent_file(input: &[u8]) -> IResult<&[u8], TorrentFile> {
    // Implement parsing for the entire torrent file here

    let mut TorrentFile = TorrentFile::new();

    let mut current_index = 1;


    if input[current_index] == '}' as u8 {
        return Ok((&input[current_index..], TorrentFile::Info(Info {})));
    }
    

    Ok((input, TorrentFile::Info(Info {})))
}