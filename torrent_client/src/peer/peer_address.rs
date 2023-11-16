#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerAddress {
    pub address: String,
    pub port: String
}

impl PeerAddress {
    pub fn new(peer_address: [u8;6]) -> PeerAddress {
        let mut address = String::new();
        for i in &peer_address[..4] {
            address.push_str(&i.to_string());
            address.push('.');
        }
        address.pop();
        let port = u16::from_be_bytes(peer_address[4..].try_into().unwrap()).to_string();

        PeerAddress{address, port}
    }
}