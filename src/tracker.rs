use anyhow::{anyhow, Context, Result};

use crate::torrent::torrent_file::TorrentFile;
use crate::torrent::TorrentContext;

use crate::utils::bencode::BencodedValue;
use crate::utils::UrlEncodable;

pub mod tracker_event;
pub use tracker_event::TrackerEvent;

mod tracker_request;
use tracker_request::TrackerRequest;

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
            None => unsafe { crate::CLIENT_OPTIONS.tracker_regular_request_interval_secs }
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

        if bencoded_response.get_from_dict(b"tracker id").is_err() {
            if let Some(last_response) = self.last_response.as_mut() {
                if let Some(last_tracker_id) = last_tracker_id {
                    last_response.insert_into_dict(b"tracker id".to_vec(), BencodedValue::ByteString(last_tracker_id));
                }
            }
        }

        Ok(bencoded_response)
    }
}