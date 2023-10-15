use std::collections::BTreeMap;

use TtTorrent::{
    torrent_file::{Sha1Hash, BencodedValue, parse_torrent_file, parse_to_torrent_file}, 
    utils::read_file_as_bytes
};

#[test]
fn test_read_file_as_bytes() {
    let path = "tests/test_torrent_1.torrent";
    let bytes = read_file_as_bytes(path).unwrap();
    assert_eq!(bytes.len(), 392);
}

#[test]
fn test_sha1hash_new() {
    let sha1_hash = Sha1Hash::new(&[90;20]);

    assert!(vec![90;20] == sha1_hash.get_hash_ref());

    let sha1_hash = Sha1Hash::new("丂丂丂丂丂丂AA".as_bytes());

    assert!("丂丂丂丂丂丂AA".as_bytes().to_vec() == sha1_hash.get_hash_ref());
}

#[test]
#[should_panic]
fn test_sha1hash_new_invalid_hash_len() {
    let sha1_hash = Sha1Hash::new(&[90;21]);

    assert!(vec![90;20] == sha1_hash.get_hash_ref());
}

#[test]
fn test_sha1hash_as_hex() {
    let sha1_hash = Sha1Hash::new(&[90;20]);

    let sha1_hash_as_hex = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";

    assert_eq!(sha1_hash_as_hex, sha1_hash.as_hex());
}

#[test]
fn test_sha1hash_as_string() {
    let sha1_hash = Sha1Hash::new(&[90;20]);
    let sha1_hash_as_string = "ZZZZZZZZZZZZZZZZZZZZ";

    assert_eq!(sha1_hash_as_string, sha1_hash.as_string());

    let sha1_hash = Sha1Hash::new("丂丂丂丂丂丂AA".as_bytes());
    let sha1_hash_as_string = "丂丂丂丂丂丂AA";

    assert_eq!(sha1_hash_as_string, sha1_hash.as_string());
}

#[test]
fn test_sha1hash_as_url_encoded() {
    let sha1_hash = Sha1Hash::new(&[90;20]);

    let sha1_hash_as_url_encoded = "ZZZZZZZZZZZZZZZZZZZZ";

    assert_eq!(sha1_hash_as_url_encoded, sha1_hash.as_url_encoded());

    let sha1_hash = Sha1Hash::new("丂丂丂丂丂丂AA".as_bytes());
    let sha1_hash_as_url_encoded = "%E4%B8%82%E4%B8%82%E4%B8%82%E4%B8%82%E4%B8%82%E4%B8%82AA";

    assert_eq!(sha1_hash_as_url_encoded, sha1_hash.as_url_encoded());
}

#[test]
fn test_bencodedvalue_get_from_dict() {
    let mut hashmap = BTreeMap::new();
    hashmap.insert(String::from("key"), BencodedValue::Integer(1));

    let bencoded_dict = BencodedValue::Dict(hashmap);

    assert_eq!(BencodedValue::Integer(1), bencoded_dict.get_from_dict(&String::from("key")));
}

#[test]
#[should_panic]
fn test_bencodedvalue_get_from_dict_invalid_key() {
    let mut hashmap = BTreeMap::new();
    hashmap.insert(String::from("key"), BencodedValue::Integer(1));

    let bencoded_dict = BencodedValue::Dict(hashmap);

    assert_eq!(BencodedValue::Integer(1), bencoded_dict.get_from_dict(&String::from("da")));
}

#[test]
#[should_panic]
fn test_bencodedvalue_get_from_dict_non_dict() {
    let bencoded_dict = BencodedValue::Integer(1);

    assert_eq!(BencodedValue::Integer(1), bencoded_dict.get_from_dict(&String::from("da")));
}

#[test]
fn test_bencodedvalue_into_dict() {

    let mut bencoded_value = BencodedValue::Dict(BTreeMap::new());

    bencoded_value.insert_into_dict("key".to_string(), BencodedValue::Integer(1));

    assert_eq!(BencodedValue::Integer(1), bencoded_value.get_from_dict(&String::from("key")));
}

#[test]
fn test_bencodedvalue_get_from_list() {
    let mut list = Vec::new();
    list.push(BencodedValue::Integer(1));

    let bencoded_list = BencodedValue::List(list);

    assert_eq!(BencodedValue::Integer(1), bencoded_list.get_from_list(0));
}

#[test]
#[should_panic]
fn test_bencodedvalue_get_from_list_invalid_index() {
    let mut list = Vec::new();
    list.push(BencodedValue::Integer(1));

    let bencoded_list = BencodedValue::List(list);

    assert_eq!(BencodedValue::Integer(1), bencoded_list.get_from_list(1));
}

#[test]
#[should_panic]
fn test_bencodedvalue_get_from_list_non_list() {
    let bencoded_list = BencodedValue::Integer(1);

    assert_eq!(BencodedValue::Integer(1), bencoded_list.get_from_list(1));
}

#[test]
fn test_bencodedvalue_into_list() {

    let mut bencoded_value = BencodedValue::List(Vec::new());

    bencoded_value.insert_into_list(BencodedValue::Integer(1));

    assert_eq!(BencodedValue::Integer(1), bencoded_value.get_from_list(0));
}

#[test]
fn test_is_valid_torrent_file() {
// one file
    let mut hashmap = BTreeMap::new();
    hashmap.insert(String::from("announce"), BencodedValue::String("da".to_string()));
    let mut info: BTreeMap<String, BencodedValue> = BTreeMap::new();
    info.insert(String::from("name"), BencodedValue::String("name".to_string()));
    info.insert(String::from("piece length"), BencodedValue::Integer(262144));
    info.insert(String::from("pieces"), BencodedValue::String("丂丂丂丂丂丂AA".to_string()));
    info.insert(String::from("length"), BencodedValue::Integer(89));

    hashmap.insert(String::from("info"), BencodedValue::Dict(info));

    let bencoded_dict = BencodedValue::Dict(hashmap);

    assert!(bencoded_dict.torrent_file_is_valid() == true);

// multiple files
    let mut hashmap = BTreeMap::new();
    hashmap.insert(String::from("announce"), BencodedValue::String("da".to_string()));
    let mut info: BTreeMap<String, BencodedValue> = BTreeMap::new();
    info.insert(String::from("name"), BencodedValue::String("name".to_string()));
    info.insert(String::from("piece length"), BencodedValue::Integer(262144));
    info.insert(String::from("pieces"), BencodedValue::String("丂丂丂丂丂丂AA".to_string()));
    
    let mut files = Vec::new();

    let mut file1 = BTreeMap::new();
    file1.insert(String::from("length"), BencodedValue::Integer(89));
    file1.insert(String::from("path"), BencodedValue::List(vec![BencodedValue::String("name".to_string())]));

    let mut file2 = BTreeMap::new();
    file2.insert(String::from("length"), BencodedValue::Integer(89));
    file2.insert(String::from("path"), BencodedValue::List(vec![BencodedValue::String("name".to_string())]));

    files.push(BencodedValue::Dict(file1));
    files.push(BencodedValue::Dict(file2));

    info.insert(String::from("files"), BencodedValue::List(files));

    hashmap.insert(String::from("info"), BencodedValue::Dict(info));

    let bencoded_dict = BencodedValue::Dict(hashmap);

    assert!(bencoded_dict.torrent_file_is_valid() == true);
}

#[test]
fn test_is_valid_torrent_file_invalid() {
    let mut hashmap = BTreeMap::new();
    hashmap.insert(String::from("announce"), BencodedValue::String("da".to_string()));

    let bencoded_dict = BencodedValue::Dict(hashmap);
    assert!(bencoded_dict.torrent_file_is_valid() == false);

    let mut hashmap = BTreeMap::new();
    hashmap.insert(String::from("announce"), BencodedValue::String("da".to_string()));
    let mut info: BTreeMap<String, BencodedValue> = BTreeMap::new();
    info.insert(String::from("name"), BencodedValue::String("name".to_string()));
    info.insert(String::from("piece length"), BencodedValue::Integer(262144));
    info.insert(String::from("pieces"), BencodedValue::String("丂丂丂丂丂丂AA".to_string()));
    info.insert(String::from("length"), BencodedValue::Integer(89));
    
    let mut files = Vec::new();

    let mut file1 = BTreeMap::new();
    file1.insert(String::from("length"), BencodedValue::Integer(89));
    file1.insert(String::from("path"), BencodedValue::List(vec![BencodedValue::String("name".to_string())]));

    let mut file2 = BTreeMap::new();
    file2.insert(String::from("length"), BencodedValue::Integer(89));
    file2.insert(String::from("path"), BencodedValue::List(vec![BencodedValue::String("name".to_string())]));

    files.push(BencodedValue::Dict(file1));
    files.push(BencodedValue::Dict(file2));

    info.insert(String::from("files"), BencodedValue::List(files));

    hashmap.insert(String::from("info"), BencodedValue::Dict(info));

    let bencoded_dict = BencodedValue::Dict(hashmap);

    assert!(bencoded_dict.torrent_file_is_valid() == false);
}

#[test]
fn test_parse_torrent_file() {
    let torrent_file = "d8:announce5:url:)4:infod6:lengthi89e4:name4:name12:piece lengthi262144e6:pieces20:丂丂丂丂丂丂AAee".as_bytes();

    let _torrent_file = parse_torrent_file(&torrent_file);

    assert!(true);
}


#[test]
#[should_panic(expected = "[Error] Invalid dictionary: doesn't have all the required keys")]
fn test_parse_torrent_file_invalid() {
    let torrent_file = "d8:announce5:url:)4:infod4:name4:name12:piece lengthi262144e6:pieces20:丂丂丂丂丂丂AAee".as_bytes();

    let _torrent_file = parse_torrent_file(&torrent_file);
}

#[test]
fn test_parse_to_torrent_file() {
    let torrent_file = "d8:announce5:url:)4:infod6:lengthi89e4:name4:name12:piece lengthi262144e6:pieces20:丂丂丂丂丂丂AAee".as_bytes();

    let torrent_file_struct = parse_torrent_file(&torrent_file);

    let new_torrent_file = parse_to_torrent_file(&torrent_file_struct);

    assert_eq!(torrent_file, new_torrent_file);
}