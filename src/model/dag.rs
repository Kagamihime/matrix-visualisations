use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;

use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::{Bfs, EdgeRef};
use petgraph::{Directed, Direction};
use serde_derive::Serialize;
use serde_json::Value as JsonValue;

use crate::cs_backend::backend::SyncResponse;

use super::event::{Event, Field};

/// The internal representation of the events DAG of the room being observed as well as various
/// informations and `HashMap`s which makes easier to locate the events.
pub struct RoomEvents {
    room_id: String,     // The ID of the room
    server_name: String, // The name of the server this DAG was retrieved from
    fields: HashSet<Field>,

    dag: Graph<Event, (), Directed>,         // The DAG of the events
    events_map: HashMap<String, NodeIndex>, // Allows to quickly locate an event in the DAG with its ID
    depth_map: HashMap<i64, Vec<NodeIndex>>, // Allows to quickly locate events at a given depth in the DAG
    pub latest_events: Vec<String>,          // The ID of the latest event in the DAG
    pub earliest_events: Vec<String>,        // The ID of the earliest event in the DAG
    max_depth: i64,                          // Minimal depth of the events in the DAG
    min_depth: i64,                          // Maximal depth of the events in the DAG
}

#[derive(Debug, Serialize)]
pub struct DataSet {
    nodes: Vec<DataSetNode>,
    edges: Vec<DataSetEdge>,
}

#[derive(Debug, Serialize)]
pub struct DataSetNode {
    pub id: String,
    pub label: String,
    pub level: i64,
    pub color: NodeColor,
}

#[derive(Debug, Serialize)]
pub struct NodeColor {
    pub border: String,
    pub background: String,
}

#[derive(Debug, Serialize)]
pub struct DataSetEdge {
    from: String,
    to: String,
}

impl RoomEvents {
    /// Creates an event DAG from the initial `SyncResponse`.
    pub fn from_sync_response(
        room_id: &str,
        server_name: &str,
        fields: &HashSet<Field>,
        res: SyncResponse,
    ) -> Option<RoomEvents> {
        match res.rooms.join.get(room_id) {
            Some(room) => {
                let timeline = parse_events(&room.timeline.events);

                let mut dag = RoomEvents {
                    room_id: room_id.to_string(),
                    server_name: server_name.to_string(),
                    fields: fields.clone(),

                    dag: Graph::new(),
                    events_map: HashMap::with_capacity(timeline.len()),
                    depth_map: HashMap::with_capacity(timeline.len()),
                    latest_events: Vec::new(),
                    earliest_events: Vec::new(),
                    max_depth: -1,
                    min_depth: -1,
                };

                dag.add_event_nodes(timeline);
                dag.update_event_edges();

                Some(dag)
            }
            None => None,
        }
    }

    /// Adds new events to the DAG from a `SyncResponse`.
    pub fn add_new_events(&mut self, res: SyncResponse) {
        if let Some(room) = res.rooms.join.get(&self.room_id) {
            let events = parse_events(&room.timeline.events);

            self.add_event_nodes(events);
            self.update_event_edges();
        }
    }

    /// Adds earlier `events` to the DAG.
    pub fn add_prev_events(&mut self, events: Vec<JsonValue>) {
        let events = parse_events(&events);

        self.add_event_nodes(events);
        self.update_event_edges();
    }

    fn add_event_nodes(&mut self, events: Vec<Event>) {
        for event in events.iter() {
            let id = &event.event_id;
            let depth = event.depth;
            let index = self.dag.add_node(event.clone()); // Add each event as a node in the DAG

            self.events_map.insert(id.clone(), index); // Update the events map

            match self.depth_map.get_mut(&depth) {
                None => {
                    self.depth_map.insert(depth, vec![index]);
                }
                Some(v) => {
                    v.push(index);
                }
            }

            if self.max_depth == -1 || depth > self.max_depth {
                self.max_depth = depth;
            }

            if self.min_depth == -1 || depth < self.min_depth {
                self.min_depth = depth;
            }
        }
    }

    fn update_event_edges(&mut self) {
        // Update the edges in the DAG
        for src_idx in self.dag.node_indices() {
            let prev_indices: Vec<NodeIndex> = self
                .dag
                .node_weight(src_idx)
                .unwrap()
                .get_prev_events()
                .iter()
                .filter(|id| self.events_map.get(**id).is_some()) // Only take into account events which are really in the DAG
                .map(|id| *self.events_map.get(*id).unwrap())
                .collect();

            for dst_idx in prev_indices {
                self.dag.update_edge(src_idx, dst_idx, ());
            }
        }

        self.latest_events.clear();
        self.earliest_events.clear();

        // Update the earliest and latest events of the DAG
        for idx in self.dag.node_indices() {
            if self.dag.edges_directed(idx, Direction::Outgoing).count() == 0 {
                let id = self.dag.node_weight(idx).unwrap().event_id.clone();

                self.earliest_events.push(id);
            }

            if self.dag.edges_directed(idx, Direction::Incoming).count() == 0 {
                let id = self.dag.node_weight(idx).unwrap().event_id.clone();

                self.latest_events.push(id);
            }
        }
    }

    pub fn get_event(&self, id: &str) -> Option<&Event> {
        self.events_map
            .get(id)
            .map(|idx| self.dag.node_weight(*idx).unwrap())
    }

    pub fn create_data_set(&mut self) -> DataSet {
        let server_name = self.server_name.clone();
        let fields = self.fields.clone();

        let nodes: Vec<DataSetNode> = self
            .dag
            .node_weights_mut()
            .map(|w| w.to_data_set_node(&server_name, &fields))
            .collect();

        let edges: Vec<DataSetEdge> = self
            .dag
            .edge_references()
            .map(|edge| {
                let from = self
                    .dag
                    .node_weight(edge.source())
                    .unwrap()
                    .event_id
                    .clone();
                let to = self
                    .dag
                    .node_weight(edge.target())
                    .unwrap()
                    .event_id
                    .clone();

                DataSetEdge { from, to }
            })
            .collect();

        DataSet { nodes, edges }
    }

    pub fn add_earlier_events_to_data_set(&self, data_set: &mut DataSet, from: Vec<String>) {
        let from_indices: HashSet<NodeIndex> = from
            .iter()
            .map(|id| *self.events_map.get(id).unwrap())
            .collect();

        let (new_node_indices, new_edges) = new_nodes_edges(&self.dag, from_indices);

        new_node_indices
            .iter()
            .map(|idx| {
                self.dag
                    .node_weight(*idx)
                    .unwrap()
                    .to_data_set_node(&self.server_name, &self.fields)
            })
            .for_each(|node| data_set.nodes.push(node));

        new_edges
            .iter()
            .map(|(src, dst)| self.to_data_set_edge((*src, *dst)).unwrap())
            .for_each(|edge| data_set.edges.push(edge));
    }

    pub fn add_new_events_to_data_set(&self, data_set: &mut DataSet, from: Vec<String>) {
        // TODO: Make a shadow copy instead of a real one
        let mut rev_dag = self.dag.clone();
        rev_dag.reverse();

        let from_indices: HashSet<NodeIndex> = from
            .iter()
            .map(|id| *self.events_map.get(id).unwrap())
            .collect();

        let (new_node_indices, rev_new_edges) = new_nodes_edges(&rev_dag, from_indices);

        // We have to reverse the edges again
        let new_edges: HashSet<(NodeIndex, NodeIndex)> = rev_new_edges
            .into_iter()
            .map(|(src, dst)| (dst, src))
            .collect();

        new_node_indices
            .iter()
            .map(|idx| {
                self.dag
                    .node_weight(*idx)
                    .unwrap()
                    .to_data_set_node(&self.server_name, &self.fields)
            })
            .for_each(|node| data_set.nodes.push(node));

        new_edges
            .iter()
            .map(|(src, dst)| self.to_data_set_edge((*src, *dst)).unwrap())
            .for_each(|edge| data_set.edges.push(edge));
    }

    fn to_data_set_edge(&self, (src, dst): (NodeIndex, NodeIndex)) -> Option<DataSetEdge> {
        let from = self.dag.node_weight(src)?.event_id.clone();
        let to = self.dag.node_weight(dst)?.event_id.clone();

        Some(DataSetEdge { from, to })
    }
}

impl DataSet {
    pub fn new() -> DataSet {
        DataSet {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
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

fn new_nodes_edges(
    dag: &Graph<Event, ()>,
    from_indices: HashSet<NodeIndex>,
) -> (HashSet<NodeIndex>, HashSet<(NodeIndex, NodeIndex)>) {
    let mut node_indices: HashSet<NodeIndex> = HashSet::from_iter(from_indices.iter().map(|i| *i));

    for &from_idx in from_indices.iter() {
        let mut bfs = Bfs::new(&dag, from_idx);

        while let Some(idx) = bfs.next(&dag) {
            node_indices.insert(idx);
        }
    }

    let new_node_indices: HashSet<NodeIndex> = node_indices
        .difference(&from_indices)
        .map(|idx| *idx)
        .collect();

    let mut new_edges: HashSet<(NodeIndex, NodeIndex)> = HashSet::new();

    for edges in new_node_indices
        .iter()
        .map(|idx| dag.edges_directed(*idx, Direction::Incoming))
    {
        for e in edges {
            new_edges.insert((e.source(), e.target()));
        }
    }

    (new_node_indices, new_edges)
}
