use anyhow::Result;

use crate::utils::sha1hash::Sha1Hash;
use crate::utils::bencode::BencodedValue;
use crate::utils::UrlEncodable;
use crate::torrent::torrent_context::TorrentContext;

use super::{Tracker, TrackerEvent};

#[derive(Debug, Clone)]
pub struct TrackerRequest {
    announce: String,
    info_hash: Sha1Hash,
    peer_id: [u8; 20],
    port: u16,
    uploaded: u64,
    downloaded: u64,
    left: u64,
    compact: u8,
    no_peer_id: u8,
    event: TrackerEvent,
    ip: Option<String>,
    numwant: Option<u32>,
    key: Option<String>,
    tracker_id: Option<String>,
}

impl TrackerRequest {
    pub async fn new(tracker: &Tracker, client_id: [u8; 20], torrent_context: &TorrentContext, tracker_event: TrackerEvent) -> Result<TrackerRequest> {
        let announce = tracker.announce.clone();
        let info_hash = torrent_context.info_hash.clone();
        let peer_id = client_id;
        let port = crate::LISTENING_PORT;
        let uploaded = torrent_context.uploaded.lock().await.clone();
        let downloaded = torrent_context.downloaded.lock().await.clone();
        let left = torrent_context.torrent_file.get_torrent_length()? - downloaded;
        let compact = 1;
        let no_peer_id = 0;
        let event = tracker_event;
        let ip = None;
        let numwant = None;
        let key = None;
        let tracker_id = tracker.last_response.as_ref().and_then(|last_response| {
            match last_response.get_from_dict(b"tracker id") {
                Ok(BencodedValue::ByteString(tracker_id)) => Some(tracker_id.as_url_encoded()),
                _ => None
            }
        });

        Ok(TrackerRequest {
            announce,
            info_hash,
            peer_id,
            port,
            uploaded,
            downloaded,
            left,
            compact,
            no_peer_id,
            event,
            ip,
            numwant,
            key,
            tracker_id,
        })
    }

    pub fn as_url(self) -> Result<String> {
        let mut url = format!{
            "{announce}?info_hash={info_hash}\
            &peer_id={peer_id}\
            &port={port}\
            &uploaded={uploaded}\
            &downloaded={downloaded}\
            &left={left}\
            &compact={compact}\
            &no_peer_id={no_peer_id}\
            &event={event}",    
            announce = self.announce,
            info_hash = self.info_hash.as_url_encoded(),
            peer_id = self.peer_id.as_url_encoded(),
            port = self.port,
            uploaded = self.uploaded,
            downloaded = self.downloaded,
            left = self.left,
            compact = self.compact,
            no_peer_id = self.no_peer_id,
            event = self.event.as_url_encoded(),
        };

        if let Some(ip) = self.ip {
            url.push_str(&format!("&ip={}", ip));
        }

        if let Some(numwant) = self.numwant {
            url.push_str(&format!("&numwant={}", numwant));
        }

        if let Some(key) = self.key {
            url.push_str(&format!("&key={}", key));
        }

        if let Some(tracker_id) = self.tracker_id {
            url.push_str(&format!("&trackerid={}", tracker_id));
        }

        Ok(url)
    }
}
