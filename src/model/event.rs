pub struct Event {
    room_id: String,          // Room identifier
    sender: String,           // The ID of the user sending the event
    origin: String,           // The `server_name` of the homeserver that created this event
    origin_server_ts: i64, // Timestamp in milliseconds on origin homeserver when this event was created
    etype: String,         // Event type
    prev_events: Vec<String>, // Event IDs for the most recent events in the room that the homeserver was aware of when it made this event
    depth: i64,               // The maximum depth of the `prev_events`, plus one
    event_id: String,         // The event ID
}
