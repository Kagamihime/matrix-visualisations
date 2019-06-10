use std::fmt;

use serde_derive::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::dag::DataSetNode;

/// The internal representation of an event in the DAG.
#[derive(Default, Clone, Deserialize, Serialize)]
pub struct Event {
    room_id: String,       // Room identifier
    sender: String,        // The ID of the user who has sent this event
    origin: String,        // The `server_name` of the homeserver which created this event
    origin_server_ts: i64, // Timestamp in milliseconds on origin homeserver when this event was created
    #[serde(rename = "type")]
    etype: String, // Event type
    prev_events: Vec<JsonValue>, // Event IDs for the most recent events in the room that the homeserver was aware of when it made this event
    pub depth: i64,              // The maximum depth of the `prev_events`, plus one
    pub event_id: String,        // The event ID
}

impl Event {
    /// This function is needed because the content of a the `prev_events` field can change
    /// across the versions of rooms.
    pub fn get_prev_events(&self) -> Vec<&str> {
        self.prev_events
            .iter()
            .map(|prev_ev| {
                if prev_ev.is_array() {
                    prev_ev[0].as_str().unwrap()
                } else {
                    prev_ev.as_str().unwrap()
                }
            })
            .collect()
    }

    pub fn to_data_set_node(&self) -> DataSetNode {
        DataSetNode {
            id: self.event_id.clone(),
            label: format!("{}", self),
            level: self.depth,
        }
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Event) -> bool {
        self.event_id == other.event_id
    }
}

impl Eq for Event {}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Sender: {}\nType: {}\nDepth: {}\nEvent ID: {}\nPrev events: {:?}",
            self.sender, self.etype, self.depth, self.event_id, self.prev_events
        )
    }
}
