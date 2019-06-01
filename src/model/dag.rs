use std::collections::HashMap;

use petgraph::dot::{Config, Dot};
use petgraph::graph::{Graph, NodeIndex};
use petgraph::Directed;
use serde_json::Value as JsonValue;

use super::event::Event;
use crate::cs_backend::backend::SyncResponse;

/// The internal representation of the events DAG of the room being observed as well as various
/// informations and `HashMap`s which makes easier to locate the events.
pub struct RoomEvents {
    room_id: String,     // The ID of the room
    server_name: String, // The name of the server this DAG was retrieved from

    dag: Graph<Event, (), Directed>,         // The DAG of the events
    events_map: HashMap<String, NodeIndex>, // Allows to quickly locate an event in the DAG with its ID
    depth_map: HashMap<i64, Vec<NodeIndex>>, // Allows to quickly locate events at a given depth in the DAG
    latest_event: String,                    // The ID of the latest event in the DAG
    earliest_event: String,                  // The ID of the earliest event in the DAG
    max_depth: i64,                          // Minimal depth of the events in the DAG
    min_depth: i64,                          // Maximal depth of the events in the DAG
}

impl RoomEvents {
    /// Creates an event DAG from the initial `SyncResponse`.
    pub fn from_sync_response(
        room_id: &str,
        server_name: &str,
        res: SyncResponse,
    ) -> Option<RoomEvents> {
        match res.rooms.join.get(room_id) {
            Some(room) => {
                let timeline = &room.timeline.events;

                let timeline = parse_events(timeline);

                let mut dag: Graph<Event, (), Directed> = Graph::new();
                let mut events_map: HashMap<String, NodeIndex> =
                    HashMap::with_capacity(timeline.len() /*+ state.len()*/);
                let mut depth_map: HashMap<i64, Vec<NodeIndex>> =
                    HashMap::with_capacity(timeline.len() /*+ state.len()*/);
                let mut latest_event = String::new();
                let mut earliest_event = String::new();
                let mut max_depth = -1;
                let mut min_depth = -1;

                for event in timeline.iter() {
                    let id = &event.event_id;
                    let depth = event.depth;
                    let index = dag.add_node(event.clone()); // Add each event as a node in the DAG

                    events_map.insert(id.clone(), index); // Update the events map

                    // Update the depth map
                    match depth_map.get_mut(&depth) {
                        None => {
                            depth_map.insert(depth, vec![index]);
                        }
                        Some(v) => {
                            v.push(index);
                        }
                    }

                    // Update the minimal and maximal depth of the events of the DAG, as well as
                    // the latest and earliest event
                    if latest_event.is_empty() {
                        latest_event = id.clone();
                        earliest_event = id.clone();
                        max_depth = depth;
                        min_depth = depth;
                    } else if let Some(latest_idx) = events_map.get(&latest_event) {
                        if let Some(latest_ev) = dag.node_weight(*latest_idx) {
                            if latest_ev < event {
                                latest_event = event.event_id.clone();
                                max_depth = event.depth;
                            }
                        }
                    } else if let Some(earliest_idx) = events_map.get(&earliest_event) {
                        if let Some(earliest_ev) = dag.node_weight(*earliest_idx) {
                            if earliest_ev > event {
                                earliest_event = event.event_id.clone();
                                min_depth = event.depth;
                            }
                        }
                    }
                }

                let edges = get_new_edges(&dag, &events_map, &depth_map, min_depth, max_depth);

                dag.extend_with_edges(edges);

                Some(RoomEvents {
                    room_id: String::from(room_id),
                    server_name: String::from(server_name),
                    dag,
                    events_map,
                    depth_map,
                    latest_event,
                    earliest_event,
                    max_depth,
                    min_depth,
                })
            }
            None => None,
        }
    }

    /// Adds new events to the DAG from a `SyncResponse`.
    pub fn add_new_events(&mut self, res: SyncResponse) {
        if let Some(room) = res.rooms.join.get(&self.room_id) {
            let old_max_depth = self.max_depth;

            let events = &room.timeline.events;

            let events = parse_events(events);

            for event in events.iter() {
                let id = &event.event_id;
                let depth = event.depth;
                let index = self.dag.add_node(event.clone()); // Add each new event as a node in the DAG

                self.events_map.insert(id.clone(), index); // Update the events map

                // Update the depth map
                match self.depth_map.get_mut(&depth) {
                    None => {
                        self.depth_map.insert(depth, vec![index]);
                    }
                    Some(v) => {
                        v.push(index);
                    }
                }

                // Update the latest event of the DAG as well as its maximal depth
                if let Some(latest_idx) = self.events_map.get(&self.latest_event) {
                    if let Some(latest_ev) = self.dag.node_weight(*latest_idx) {
                        if latest_ev < event {
                            self.latest_event = event.event_id.clone();
                            self.max_depth = event.depth;
                        }
                    }
                }
            }

            // Get the new egdes of the DAG
            let edges = get_new_edges(
                &self.dag,
                &self.events_map,
                &self.depth_map,
                old_max_depth,
                self.max_depth,
            );

            self.dag.extend_with_edges(edges);
        }
    }

    /// Adds earlier `events` to the DAG.
    pub fn add_prev_events(&mut self, events: Vec<JsonValue>) {
        let old_min_depth = self.min_depth;

        let events = parse_events(&events);

        for event in events.iter() {
            let id = &event.event_id;
            let depth = event.depth;
            let index = self.dag.add_node(event.clone()); // Add each earlier event as a node in the DAG

            self.events_map.insert(id.clone(), index); // Update the events map

            // Update the depth map
            match self.depth_map.get_mut(&depth) {
                None => {
                    self.depth_map.insert(depth, vec![index]);
                }
                Some(v) => {
                    v.push(index);
                }
            }

            // Update the earliest event of the DAG as well as its minimal depth
            if let Some(earliest_idx) = self.events_map.get(&self.earliest_event) {
                if let Some(earliest_ev) = self.dag.node_weight(*earliest_idx) {
                    if earliest_ev > event {
                        self.earliest_event = event.event_id.clone();
                        self.min_depth = event.depth;
                    }
                }
            }
        }

        // Get the new egdes of the DAG
        let edges = get_new_edges(
            &self.dag,
            &self.events_map,
            &self.depth_map,
            self.min_depth,
            old_min_depth,
        );

        self.dag.extend_with_edges(edges);
    }

    pub fn to_dot(&self) -> String {
        format!("{:?}", Dot::with_config(&self.dag, &[Config::EdgeNoLabel]))
    }
}

// Parses a list of events encoded as JSON values.
fn parse_events(json_events: &Vec<JsonValue>) -> Vec<Event> {
    json_events
        .iter()
        .map(|ev| {
            serde_json::from_value(ev.clone()).expect(&format!(
                "Failed to parse event:\n{}",
                serde_json::to_string_pretty(&ev).expect("Failed to fail..."),
            ))
        })
        .collect()
}

// Computes the list of the missing edges in the DAG.
fn get_new_edges(
    dag: &Graph<Event, (), Directed>,
    events_map: &HashMap<String, NodeIndex>,
    depth_map: &HashMap<i64, Vec<NodeIndex>>,
    min_depth: i64,
    max_depth: i64,
) -> Vec<(NodeIndex, NodeIndex)> {
    let mut edges = Vec::new();

    for d in (min_depth..=max_depth).rev() {
        if let Some(indices) = depth_map.get(&d) {
            for idx in indices {
                if let Some(src_ev) = dag.node_weight(*idx) {
                    for dst_id in src_ev.get_prev_events() {
                        if let Some(dst_idx) = events_map.get(dst_id) {
                            if dag.find_edge(*idx, *dst_idx).is_none() {
                                edges.push((*idx, *dst_idx));
                            }
                        }
                    }
                }
            }
        }
    }

    edges
}
