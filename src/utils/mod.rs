
pub fn read_bytes_as_string(input: &[u8]) -> String {
    input.iter().map(|&c| c as char).collect()
}