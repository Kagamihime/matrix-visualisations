use std::cmp::Ordering;

use serde_derive::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Event {
    room_id: String,       // Room identifier
    sender: String,        // The ID of the user sending the event
    origin: String,        // The `server_name` of the homeserver that created this event
    origin_server_ts: i64, // Timestamp in milliseconds on origin homeserver when this event was created
    #[serde(rename = "type")]
    etype: String, // Event type
    prev_events: Vec<JsonValue>, // Event IDs for the most recent events in the room that the homeserver was aware of when it made this event
    pub depth: i64,              // The maximum depth of the `prev_events`, plus one
    pub event_id: String,        // The event ID
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Event) -> Option<Ordering> {
        if self.depth == other.depth {
            Some(self.origin_server_ts.cmp(&other.origin_server_ts))
        } else {
            Some(self.depth.cmp(&other.depth))
        }
    }
}

impl Ord for Event {
    fn cmp(&self, other: &Event) -> Ordering {
        if self.depth == other.depth {
            self.origin_server_ts.cmp(&other.origin_server_ts)
        } else {
            self.depth.cmp(&other.depth)
        }
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Event) -> bool {
        self.event_id == other.event_id
    }
}

impl Eq for Event {}
