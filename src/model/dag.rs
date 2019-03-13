use std::collections::HashMap;

use petgraph::graph::{Graph, NodeIndex};
use petgraph::Directed;

use super::event::Event;

pub struct RoomEvents {
    room_id: String,     // The ID of the room
    server_name: String, // The name of the server this DAG was retrieved from

    dag: Graph<Event, (), Directed>,        // The DAG of the events
    events_map: HashMap<String, NodeIndex>, // Allows to quickly locate an event in the DAG with its ID
    latest_event: String,                   // The ID of the latest event in the DAG
}
