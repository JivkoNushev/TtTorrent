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