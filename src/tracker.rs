use anyhow::{anyhow, Context, Result};

use crate::torrent::torrent_file::TorrentFile;
use crate::torrent::TorrentContext;

use crate::utils::sha1hash::Sha1Hash;
use crate::utils::bencode::BencodedValue;
use crate::utils::UrlEncodable;

#[derive(Debug, Clone)]
pub enum TrackerEvent {
    Started,
    Stopped,
    Completed,
    None,
}

impl UrlEncodable for TrackerEvent {
    fn as_url_encoded(&self) -> String {
        match self {
            TrackerEvent::Started => "started",
            TrackerEvent::Stopped => "stopped",
            TrackerEvent::Completed => "completed",
            TrackerEvent::None => "",
        }.to_string()
    }
}


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
    async fn new(tracker: &Tracker, client_id: [u8; 20], torrent_context: &TorrentContext, tracker_event: TrackerEvent) -> Result<TrackerRequest> {
        let announce = tracker.announce.clone();
        let info_hash = torrent_context.info_hash.clone();
        let peer_id = client_id;
        let port = crate::SEEDING_PORT;
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

    fn as_url(self) -> Result<String> {
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



#[derive(Debug, Clone)]
pub struct Tracker {
    announce: String,
    last_response: Option<BencodedValue>,
}

impl Tracker {
    pub fn get_interval(&self) -> u64 {
        match self.last_response.as_ref().and_then(|last_response| {
            match last_response.get_from_dict(b"interval") {
                Ok(BencodedValue::Integer(interval)) => Some(interval),
                _ => None
            }
        }) {
            Some(interval) => interval as u64,
            None => crate::TRACKER_REGULAR_REQUEST_INTERVAL_SECS
        }
    }

    pub fn from_torrent_file(torrent_file: &TorrentFile) -> Result<Tracker> {
        let tracker_announce = torrent_file.get_bencoded_dict_ref().get_from_dict(b"announce")?;
    
        let tracker_announce = match tracker_announce {
            BencodedValue::ByteString(tracker_announce) => tracker_announce,
            _ => return Err(anyhow!("No tracker announce key found"))
        };
    
        let announce = String::from_utf8(tracker_announce.clone())?;
    
        Ok(Tracker {
            announce,
            last_response: None,
        })
    }

    pub async fn response(&mut self, client_id: [u8; 20], torrent_context: &TorrentContext, tracker_event: TrackerEvent) -> Result<BencodedValue> {
        let request = TrackerRequest::new(self, client_id, torrent_context, tracker_event).await.context("creating tracker request")?.as_url()?;
        tracing::debug!("request: {}", request);
        
        let response = reqwest::get(request).await.context("invalid tracker url")?;
        let response_bytes = response.bytes().await.context("error getting response bytes")?; 
        tracing::debug!("response: {:?}", response_bytes.to_vec().as_url_encoded()); 
        
        let bencoded_response = BencodedValue::from_bytes(&response_bytes).context("creating bencoded response")?;

        let last_tracker_id = self.last_response.as_ref().and_then(|last_response| {
            match last_response.get_from_dict(b"tracker id") {
                Ok(BencodedValue::ByteString(tracker_id)) => Some(tracker_id),
                _ => None
            }
        });

        self.last_response = Some(bencoded_response.clone());

        if let Err(_) = bencoded_response.get_from_dict(b"tracker id") {
            self.last_response.as_mut().and_then(|last_response| {
                if let Some(last_tracker_id) = last_tracker_id {
                    last_response.insert_into_dict(b"tracker id".to_vec(), BencodedValue::ByteString(last_tracker_id));
                }

                Some(())
            });
        }

        Ok(bencoded_response)
    }
}