use crate::torrent::torrent_file::Sha1Hash;

pub struct Handshake {
    pub protocol_len: u8,
    pub protocol: [u8; 19],
    pub reserved: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: Sha1Hash, peer_id: [u8; 20]) -> Handshake {
        Handshake {
            protocol_len: 19,
            protocol: *b"BitTorrent protocol",
            reserved: [0; 8],
            info_hash: info_hash.get_hash_ref().clone(),
            peer_id: peer_id,
        }
    }

    pub fn to_bytes(&self) -> [u8; 68] {
        let mut handshake: Vec<u8> = Vec::new();
        handshake.push(self.protocol_len);
        handshake.append(&mut self.protocol.to_vec());
        handshake.append(&mut self.reserved.to_vec());
        handshake.append(&mut self.info_hash.to_vec());
        handshake.append(&mut self.peer_id.to_vec());

        handshake.try_into().unwrap()
    }
}