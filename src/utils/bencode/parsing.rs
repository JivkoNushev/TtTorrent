use anyhow::Result;

use super::super::BencodedValue;

mod parsing_utils;

pub fn encode(bytes: &[u8]) -> Result<BencodedValue> {
    parsing_utils::parse_to_bencoded_value(bytes)
}

pub fn decode(bencoded_value: &BencodedValue) -> Result<Vec<u8>> {
    parsing_utils::parse_from_bencoded_value(bencoded_value)
}

mod torrent_parser_tests {
    
    #[allow(unused_imports)]
    use parsing_utils::*;
    use super::*;
    
    #[test]
    fn test_parse_integer() {
        let input = b"123";
        let result = parse_integer(input).unwrap();
        assert_eq!(result, 123);

        let input = b"1234567891234567891";
        let result = parse_integer(input).unwrap();
        assert_eq!(result, 1234567891234567891);

        let input = b"0";
        let result = parse_integer(input).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_parse_integer_negative_int() {
        let input = b"-123";
        if let Err(e) = parse_integer(input) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid integer: empty input")]
    fn test_parse_integer_empty_input() {
        let input = b"";
        if let Err(e) = parse_integer(input) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid integer: leading zeros")]
    fn test_parse_integer_leading_zeros() {
        let input = b"0123";
        if let Err(e) = parse_integer(input) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid integer: parsing an empty number")]
    fn test_parse_integer_non_number() {
        let input = b"abc";
        if let Err(e) = parse_integer(input) {
            panic!("{}", e);
        }
    }

    #[test]
    fn test_parse_bencoded_integer() {
        let input = b"i123e";
        let result = parse_bencoded_integer(input).unwrap();
        assert_eq!(result, 123);

        let input = b"i1234567891234567891e";
        let result = parse_bencoded_integer(input).unwrap();
        assert_eq!(result, 1234567891234567891);

        let input = b"i0e";
        let result = parse_bencoded_integer(input).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_parse_bencoded_integer_negative_int() {
        let input = b"i-123e";
        if let Err(e) = parse_bencoded_integer(input) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: missing 'i' prefix")]
    fn test_parse_bencoded_integer_missing_i() {
        let input = b"123e";
        if let Err(e) = parse_bencoded_integer(input) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: missing 'e' suffix")]
    fn test_parse_bencoded_integer_missing_e() {
        let input = b"i123d";
        if let Err(e) = parse_bencoded_integer(input) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: parsing an empty number")]
    fn test_parse_bencoded_integer_empty_number() {
        let input = b"ie";
        if let Err(e) = parse_bencoded_integer(input) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: leading zeros")]
    fn test_parse_bencoded_integer_leading_zeros() {
        let input = b"i0123e";
        if let Err(e) = parse_bencoded_integer(input) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: empty input")]
    fn test_parse_bencoded_integer_empty_input() {
        let input = b"";
        if let Err(e) = parse_bencoded_integer(input) {
            panic!("{}", e);
        }
    }

    #[test]
    fn test_create_int() {
        let input = b"i123e";
        let mut cur_index = 0;
        let result = create_int(input, &mut cur_index).unwrap();
        assert_eq!(result, BencodedValue::Integer(123));
        assert_eq!(cur_index, 5);

        let input = b"i1234567891234567891e";
        let mut cur_index = 0;
        let result = create_int(input, &mut cur_index).unwrap();
        assert_eq!(result, BencodedValue::Integer(1234567891234567891));
        assert_eq!(cur_index, 21);

        let input = b"i0e";
        let mut cur_index = 0;
        let result = create_int(input, &mut cur_index).unwrap();
        assert_eq!(result, BencodedValue::Integer(0));
        assert_eq!(cur_index, 3);
    }

    #[test]
    fn test_create_int_negative_int() {
        let input = b"i-123e";
        if let Err(e) = create_int(input, &mut 0) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: missing 'i' prefix")]
    fn test_create_int_missing_i() {
        let input = b"123e";
        if let Err(e) = create_int(input, &mut 0) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: missing 'e' suffix")]
    fn test_create_int_missing_e() {
        let input = b"i123d";
        if let Err(e) = create_int(input, &mut 0) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: parsing an empty number")]
    fn test_create_int_empty_number() {
        let input = b"ie";
        if let Err(e) = create_int(input, &mut 0) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: leading zeros")]
    fn test_create_int_leading_zeros() {
        let input = b"i0123e";
        if let Err(e) = create_int(input, &mut 0) {
            panic!("{}", e);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid bencoded integer: empty input")]
    fn test_create_int_empty_input() {
        let input = b"";
        if let Err(e) = create_int(input, &mut 0) {
            panic!("{}", e);
        }
    }

    #[test]
    fn test_create_dict() {
        let torrent_file = "d8:announce5:url:)4:infod4:name4:name12:piece lengthi262144e6:pieces20:丂丂丂丂丂丂AA6:lengthi89eee".as_bytes();

        match create_dict(&torrent_file, &mut 0) {
            Ok(_) => assert!(true),
            Err(e) => panic!("{}", e)
        }
    }

    #[test]
    #[should_panic(expected = "Invalid torrent file: too short")]
    fn test_create_dict_torrent_file_too_short() {
        let torrent_file = "d8:announce5:url:)3:inti89e".as_bytes();

        if let Err(e) = create_dict(&torrent_file, &mut 0) {
            panic!("{}", e);
        }
    }

    #[test]
    fn test_create_list() {
        let torrent_file = "l4:spami90elee".as_bytes();

        let list = create_list(&torrent_file, &mut 0).unwrap();
        let list_valid = BencodedValue::List(vec![BencodedValue::ByteString("spam".as_bytes().to_vec()), BencodedValue::Integer(90), BencodedValue::List(Vec::new())]);

        assert_eq!(list_valid, list);
    }

    #[test]
    #[should_panic(expected = "Invalid torrent file: too short")]
    fn test_create_list_torrent_file_too_short() {
        let torrent_file = "l4:spami90ele".as_bytes();

        let list = create_list(&torrent_file, &mut 0).unwrap();
        let list_valid = BencodedValue::List(vec![BencodedValue::ByteString("spam".as_bytes().to_vec()), BencodedValue::Integer(90), BencodedValue::List(Vec::new())]);

        assert_eq!(list_valid, list);
    }

    #[test]
    fn test_to_bencoded_dict() {
        let torrent_file = "d8:announce5:url:)4:infod4:name4:name12:piece lengthi262144e6:pieces20:丂丂丂丂丂丂AA6:lengthi89eee".as_bytes();
        let dict: BencodedValue = BencodedValue::from_bytes(torrent_file).unwrap();

        let torrent_file = BencodedValue::as_bytes(&dict).unwrap();

        let new_torrent_file = String::from_utf8(torrent_file).unwrap();

        let _new_dict: BencodedValue = BencodedValue::from_bytes(new_torrent_file.as_bytes()).unwrap();

        assert!(true);
    }

    #[test]
    fn test_to_bencoded_list() {
        let torrent_file = "l4:spami90elee".as_bytes();

        let list = create_list(&torrent_file, &mut 0).unwrap();

        let bencoded_list = to_bencoded_list(&list).unwrap();

        assert_eq!(bencoded_list, torrent_file);
    }

}