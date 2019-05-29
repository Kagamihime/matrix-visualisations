use std::collections::HashMap;

use petgraph::dot::{Config, Dot};
use petgraph::graph::{Graph, NodeIndex};
use petgraph::Directed;
use serde_json::Value as JsonValue;

use super::event::Event;
use crate::cs_backend::backend::SyncResponse;

pub struct RoomEvents {
    room_id: String,     // The ID of the room
    server_name: String, // The name of the server this DAG was retrieved from

    dag: Graph<Event, (), Directed>,        // The DAG of the events
    events_map: HashMap<String, NodeIndex>, // Allows to quickly locate an event in the DAG with its ID
    depth_map: HashMap<i64, Vec<NodeIndex>>,
    latest_event: String,   // The ID of the latest event in the DAG
    earliest_event: String, // The ID of the earliest event in the DAG
    max_depth: i64,
    min_depth: i64,
}

impl RoomEvents {
    pub fn from_sync_response(
        room_id: &str,
        server_name: &str,
        res: SyncResponse,
    ) -> Option<RoomEvents> {
        match res.rooms.join.get(room_id) {
            Some(room) => {
                let timeline = &room.timeline.events;

                let timeline: Vec<Event> = timeline
                    .iter()
                    .map(|ev| {
                        serde_json::from_value(ev.clone()).expect(&format!(
                            "Failed to parse timeline event:\n{}",
                            serde_json::to_string_pretty(&ev).expect("Failed to fail..."),
                        ))
                    })
                    .collect();

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
                    let index = dag.add_node(event.clone());

                    events_map.insert(id.clone(), index);

                    match depth_map.get_mut(&depth) {
                        None => {
                            depth_map.insert(depth, vec![index]);
                        }
                        Some(v) => {
                            v.push(index);
                        }
                    }

                    if latest_event.is_empty() {
                        latest_event = id.clone();
                        earliest_event = id.clone();
                        max_depth = depth;
                        min_depth = depth;
                    } else if let Some(latest_index) = events_map.get(&latest_event) {
                        if let Some(latest_ev) = dag.node_weight(*latest_index) {
                            if latest_ev < event {
                                latest_event = event.event_id.clone();
                                max_depth = event.depth;
                            }
                        }
                    } else if let Some(earliest_index) = events_map.get(&earliest_event) {
                        if let Some(earliest_ev) = dag.node_weight(*earliest_index) {
                            if earliest_ev > event {
                                earliest_event = event.event_id.clone();
                                min_depth = event.depth;
                            }
                        }
                    }
                }

                let mut edges: Vec<(NodeIndex, NodeIndex)> = Vec::new();

                // TODO: Refactor with iterators
                for d in (min_depth..=max_depth).rev() {
                    if let Some(indices) = depth_map.get(&d) {
                        for index in indices {
                            if let Some(src_ev) = dag.node_weight(*index) {
                                for dst_id in src_ev.get_prev_events() {
                                    if let Some(dst_index) = events_map.get(dst_id) {
                                        edges.push((*index, *dst_index));
                                    }
                                }
                            }
                        }
                    }
                }

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

    pub fn add_prev_events(&mut self, events: Vec<JsonValue>) {
        let old_min_depth = self.min_depth;

        let events: Vec<Event> = events
            .iter()
            .map(|ev| {
                serde_json::from_value(ev.clone()).expect(&format!(
                    "Failed to parse prev event:\n{}",
                    serde_json::to_string_pretty(&ev).expect("Failed to fail..."),
                ))
            })
            .collect();

        for event in events.iter() {
            let id = &event.event_id;
            let depth = event.depth;
            let index = self.dag.add_node(event.clone());

            self.events_map.insert(id.clone(), index);

            match self.depth_map.get_mut(&depth) {
                None => {
                    self.depth_map.insert(depth, vec![index]);
                }
                Some(v) => {
                    v.push(index);
                }
            }

            if let Some(earliest_index) = self.events_map.get(&self.earliest_event) {
                if let Some(earliest_ev) = self.dag.node_weight(*earliest_index) {
                    if earliest_ev > event {
                        self.earliest_event = event.event_id.clone();
                        self.min_depth = event.depth;
                    }
                }
            }
        }

        let mut edges: Vec<(NodeIndex, NodeIndex)> = Vec::new();

        // TODO: Refactor with iterators
        for d in (self.min_depth..=old_min_depth).rev() {
            if let Some(indices) = self.depth_map.get(&d) {
                for index in indices {
                    if let Some(src_ev) = self.dag.node_weight(*index) {
                        for dst_id in src_ev.get_prev_events() {
                            if let Some(dst_index) = self.events_map.get(dst_id) {
                                if self.dag.find_edge(*index, *dst_index).is_none() {
                                    edges.push((*index, *dst_index));
                                }
                            }
                        }
                    }
                }
            }
        }

        self.dag.extend_with_edges(edges);
    }

    pub fn to_dot(&self) -> String {
        format!("{:?}", Dot::with_config(&self.dag, &[Config::EdgeNoLabel]))
    }
}
