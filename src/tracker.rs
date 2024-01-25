use anyhow::{anyhow, Result, Context};

use crate::torrent::torrent_file::{ BencodedValue, Sha1Hash, TorrentFile };
use crate::torrent::{TorrentContext, self};

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
    trackerid: Option<String>,
}

impl TrackerRequest {
    async fn new(announce: String, client_id: [u8; 20], torrent_context: &TorrentContext, tracker_event: TrackerEvent) -> Result<TrackerRequest> {
        let info_hash = torrent_context.info_hash.clone();
        let peer_id = client_id;
        let port = 6881;

        // calculate uploaded, downloaded, and left
        let uploaded = torrent_context.uploaded.lock().await.clone();
        let downloaded = torrent_context.downloaded.lock().await.clone();
        let left = torrent_context.torrent_file.get_torrent_length()? - downloaded;
        let compact = 1;
        let no_peer_id = 0;
        let event = tracker_event;
        let ip = None;
        let numwant = None;
        let key = None;
        let trackerid = None;

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
            trackerid,
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

        if let Some(trackerid) = self.trackerid {
            url.push_str(&format!("&trackerid={}", trackerid));
        }

        Ok(url)
    }

    fn start(announce: String, client_id: [u8; 20], torrent_context: &TorrentContext) -> Result<TrackerRequest> {
        let info_hash = torrent_context.info_hash.clone();
        let peer_id = client_id;
        let port = 6881;
        let uploaded = 0;
        let downloaded = 0;
        let left = torrent_context.torrent_file.get_torrent_length()?;
        let compact = 1;
        let no_peer_id = 0;
        let event = TrackerEvent::Started;
        let ip = None;
        let numwant = None;
        let key = None;
        let trackerid = None;

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
            trackerid,
        })
    }
}



#[derive(Debug, Clone)]
pub struct Tracker {
    announce: String,
}

impl Tracker {
    pub fn from_torrent_file(torrent_file: &TorrentFile) -> Result<Tracker> {
        let tracker_announce = torrent_file.get_bencoded_dict_ref().get_from_dict("announce")?;
    
        let tracker_announce = match tracker_announce {
            BencodedValue::ByteString(tracker_announce) => tracker_announce,
            _ => return Err(anyhow!("No tracker announce key found"))
        };
    
        let announce = String::from_utf8(tracker_announce.clone())?;
    
        Ok(Tracker {
            announce,
        })
    }

    fn started_request(&mut self, client_id: [u8; 20], torrent_context: &TorrentContext) -> Result<String> {
        let request = TrackerRequest::start(self.announce.clone(), client_id, torrent_context)?;
        request.as_url()
    }

    async fn new_request(&mut self, client_id: [u8; 20], torrent_context: &TorrentContext, tracker_event: TrackerEvent) -> Result<String> {
        let request = TrackerRequest::new(self.announce.clone(), client_id, torrent_context, tracker_event).await?;
        request.as_url()
    }

    pub async fn regular_response(&mut self, client_id: [u8; 20], torrent_context: &TorrentContext) -> Result<reqwest::Response> {
        // if no pieces have been downloaded, send a started request
        let request = if torrent_context.needed.lock().await.needed_blocks.len() == torrent_context.torrent_info.blocks_count {
            self.started_request(client_id, torrent_context)?
        }
        else {
            self.new_request(client_id, torrent_context, TrackerEvent::None).await?
        };

        let response = match crate::DEBUG_MODE {
            true => reqwest::get("https://1.1.1.1").await.context("error with debug request")?,
            false => reqwest::get(request).await.context("invalid tracker url")?
        };

        Ok(response)
    }

    pub async fn stopped_response(&mut self, client_id: [u8; 20], torrent_context: &TorrentContext) -> Result<reqwest::Response> {
        let request = self.new_request(client_id, torrent_context, TrackerEvent::Stopped).await?;

        let response = match crate::DEBUG_MODE {
            true => reqwest::get("https://1.1.1.1").await.context("error with debug request")?,
            false => reqwest::get(request).await.context("invalid tracker url")?
        };

        Ok(response)

    }

    pub async fn completed_response(&mut self, client_id: [u8; 20], torrent_context: &TorrentContext) -> Result<reqwest::Response> {
        let request = self.new_request(client_id, torrent_context, TrackerEvent::Completed).await?;

        let response = match crate::DEBUG_MODE {
            true => reqwest::get("https://1.1.1.1").await.context("error with debug request")?,
            false => reqwest::get(request).await.context("invalid tracker url")?
        };

        Ok(response)
    }
}