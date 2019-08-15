use std::sync::{Arc, RwLock};

use serde_derive::Serialize;
use stdweb::web;
use stdweb::web::IParentNode;
use stdweb::Value;

use crate::model::dag::RoomEvents;
use crate::model::dag::{DataSet, OrphanInfo};
use crate::BackendChoice;

/// This struct contains the DAG displayed by the application.
///
/// `data` contains every nodes of every views of the DAG, so they are displayed side-by-side
/// within the same vis.js network. Each node stored there has a prefix of the form "subdag_X_"
/// which means that this node belongs to the view X.
///
/// Each of `earliest_events`, `latest_events` and `orphan_events` variables contains a list of
/// lists of the earliest/latest/orphan events' IDs currently displayed for each views. So
/// `*_events[X]` corresponds with the view X.
pub struct VisJsService {
    lib: Option<Value>,
    network: Option<Value>,
    bk_type: Arc<RwLock<BackendChoice>>,
    data: Option<Value>,
    earliest_events: Vec<Vec<String>>,
    latest_events: Vec<Vec<String>>,
    orphan_events: Vec<Vec<OrphanInfo>>,
}

// This enables the serialization of the ID of a view, so it can be used within the `js!`
// macro.
#[derive(Clone, Copy, Serialize)]
struct ViewId {
    id: usize,
}

impl VisJsService {
    pub fn new(bk_type: Arc<RwLock<BackendChoice>>) -> Self {
        let lib = js! {
            return vis;
        };

        VisJsService {
            lib: Some(lib),
            network: None,
            bk_type,
            data: None,
            earliest_events: Vec::new(),
            latest_events: Vec::new(),
            orphan_events: Vec::new(),
        }
    }

    /// Creates and initialises the vis.js network as well as its parameters and callback
    /// functions for interacting with it.
    pub fn init(
        &mut self,
        container_id: &str,
        targeted_view_input_id: &str,
        more_ev_btn_id: &str,
        selected_event_input_id: &str,
        display_body_btn_id: &str,
        ancestors_input_id: &str,
        ancestors_btn_id: &str,
    ) {
        let lib = self.lib.as_ref().expect("vis library object lost");

        let container = web::document()
            .query_selector(container_id)
            .expect("Couldn't get document element")
            .expect("Couldn't get document element");
        let targeted_view_input = web::document()
            .query_selector(targeted_view_input_id)
            .expect("Couldn't get document element")
            .expect("Couldn't get document element");
        let more_ev_btn = web::document()
            .query_selector(more_ev_btn_id)
            .expect("Couldn't get document element")
            .expect("Couldn't get document element");
        let selected_event_input = web::document()
            .query_selector(selected_event_input_id)
            .expect("Couldn't get document element")
            .expect("Couldn't get document element");
        let display_body_btn = web::document()
            .query_selector(display_body_btn_id)
            .expect("Couldn't get document element")
            .expect("Couldn't get document element");
        let ancestors_input = web::document()
            .query_selector(ancestors_input_id)
            .expect("Couldn't get document element")
            .expect("Couldn't get document element");
        let ancestors_btn = web::document()
            .query_selector(ancestors_btn_id)
            .expect("Couldn't get document element")
            .expect("Couldn't get document element");

        js_serializable!(DataSet);
        js_serializable!(OrphanInfo);
        js_serializable!(ViewId);

        self.data = Some(js! {
            var nodes = new vis.DataSet({});
            var edges = new vis.DataSet({});

            var data = {
                nodes: nodes,
                edges: edges
            };

            return data;
        });

        self.network = Some(js! {
            var vis = @{lib};

            var data = @{&self.data};

            var options = {
                layout: {
                    randomSeed: undefined,
                    improvedLayout: true,
                    hierarchical: {
                        enabled: true,
                        levelSeparation: 250,
                        nodeSpacing: 500,
                        treeSpacing: 300,
                        sortMethod: "directed"
                    }
                },
                nodes: {
                    shape: "box",
                    widthConstraint: {
                        minimum: 200,
                        maximum: 300
                    }
                },
                edges: {
                    arrows: "to",
                    smooth: true
                },
                interaction: {
                    dragNodes: false
                },
                physics: {
                    enabled: false
                }
            };

            var network = new vis.Network(@{container}, data, options);

            function select_node() {
                let id = network.getSelectedNodes()[0];

                if (id.endsWith("_more_ev")) {
                    let split_id = id.split("_");
                    let targeted_view_input = @{targeted_view_input.clone()};

                    targeted_view_input.value = split_id[1];
                    @{more_ev_btn}.click();
                }

                if (id.includes("_more_of_")) {
                    let split_id = id.split("_");
                    let targeted_view_input = @{targeted_view_input.clone()};
                    let id_input = @{ancestors_input};

                    let pref_patt = new RegExp("subdag_[0-9]+_more_of_");

                    targeted_view_input.value = split_id[1];
                    id_input.value = id.replace(pref_patt, "");
                    @{ancestors_btn}.click();
                }
            }

            network.on("selectNode", select_node);

            function display_json_body(ev) {
                let id = ev.nodes[0];
                let split_id = id.split("_");

                // Only display body if the node matches with an actual event
                if (split_id[2].startsWith("$")) {
                    let targeted_view_input = @{targeted_view_input};
                    let id_input = @{selected_event_input};

                    let pref_patt = new RegExp("subdag_[0-9]+_");

                    targeted_view_input.value = split_id[1];
                    id_input.value = id.replace(pref_patt, "");
                    @{display_body_btn}.click();
                }
            }

            network.on("doubleClick", display_json_body);

            return network;
        });
    }

    /// Adds a new `events_dag` for the view `view_id`.
    pub fn add_dag(&mut self, events_dag: Arc<RwLock<RoomEvents>>, view_id: usize) {
        let backend = *self.bk_type.read().unwrap();
        let events_dag = events_dag.read().unwrap();

        let data = self.data.as_ref().expect("No data set found");
        let mut events = events_dag.create_data_set();
        events.add_prefix(&format!("subdag_{}_", view_id));

        while self.earliest_events.len() <= view_id {
            self.earliest_events.push(Vec::new());
        }
        while self.latest_events.len() <= view_id {
            self.latest_events.push(Vec::new());
        }
        while self.orphan_events.len() <= view_id {
            self.orphan_events.push(Vec::new());
        }

        self.earliest_events[view_id] = events_dag.earliest_events.clone();
        self.latest_events[view_id] = events_dag.latest_events.clone();
        self.orphan_events[view_id] = events_dag.orphan_events.clone();

        let view_id = ViewId { id: view_id };

        match backend {
            BackendChoice::CS => {
                self.data = Some(js! {
                    var view_id = @{view_id};
                    var data = @{data};
                    var events = @{events};

                    var min_depth = -1;
                    for (let n of events.nodes) {
                        if (min_depth == -1 || n["level"] < min_depth) {
                            min_depth = n["level"];
                        }
                    }

                    data.nodes.add(events.nodes);
                    data.edges.add(events.edges);

                    // Add the button to load more events
                    data.nodes.add({
                        id: "subdag_" + view_id.id + "_more_ev",
                        label: "Load more events",
                        level: min_depth - 1
                    });
                    for (let ev of @{&self.earliest_events[view_id.id]}) {
                        data.edges.add({
                            id: "subdag_" + view_id.id + "_" + ev + "_more_ev",
                            from: "subdag_" + view_id.id + "_" + ev,
                            to: "subdag_" + view_id.id + "_more_ev"
                        });
                    }

                    return data;
                });
            }
            BackendChoice::Postgres => {
                self.data = Some(js! {
                    var view_id = @{view_id};
                    var data = @{data};
                    var events = @{events};

                    data.nodes.add(events.nodes);
                    data.edges.add(events.edges);

                    // Add the buttons to load ancestors
                    for (let ev of @{&self.orphan_events[view_id.id]}) {
                        data.nodes.add({
                            id: "subdag_" + view_id.id + "_more_of_" + ev.id,
                            label: "Load ancestors",
                            level: ev.depth - 1
                        });

                        data.edges.add({
                            id: "subdag_" + view_id.id + "_" + ev.id + "_more_of",
                            from: "subdag_" + view_id.id + "_" + ev.id,
                            to: "subdag_" + view_id.id + "_more_of_" + ev.id
                        });
                    }

                    return data;
                });
            }
        }
    }

    /// Removes the DAG of the view `view_id`.
    pub fn remove_dag(&mut self, view_id: usize) {
        let data = self.data.as_ref().expect("No data set found");

        self.earliest_events[view_id] = Vec::new();
        self.latest_events[view_id] = Vec::new();
        self.orphan_events[view_id] = Vec::new();

        let view_id = ViewId { id: view_id };

        self.data = Some(js! {
            var view_id = @{view_id};
            var data = @{data};

            for (let node of data.nodes.get()) {
                if (node.id.startsWith("subdag_" + view_id.id + "_")) {
                    data.nodes.remove(node.id);
                }
            }

            for (let edge of data.edges.get()) {
                if (edge.id.startsWith("subdag_" + view_id.id + "_")) {
                    data.edges.remove(edge.id);
                }
            }

            return data;
        });
    }

    /// Updates the DAG of the view `view_id` so that each additional events in `events_dag`
    /// is added to the vis.js network.
    pub fn update_dag(&mut self, events_dag: Arc<RwLock<RoomEvents>>, view_id: usize) {
        let events_dag = events_dag.read().unwrap();
        let backend = *self.bk_type.read().unwrap();

        if self.earliest_events[view_id] != events_dag.earliest_events {
            let old_earliest_events = self.earliest_events[view_id].clone();
            let new_earliest_events = events_dag.earliest_events.clone();
            let old_orphan_events = self.orphan_events[view_id].clone();
            let new_orphan_events = events_dag.orphan_events.clone();

            let data = self.data.as_ref().expect("No data set found");

            let mut earlier_events = DataSet::new();
            events_dag
                .add_earlier_events_to_data_set(&mut earlier_events, old_earliest_events.clone());
            earlier_events.add_prefix(&format!("subdag_{}_", view_id));

            let view_id = ViewId { id: view_id };

            match backend {
                BackendChoice::CS => {
                    self.data = Some(js! {
                        var view_id = @{view_id};
                        var data = @{data};
                        var ev = @{earlier_events};

                        var min_depth = -1;
                        for (let n of ev.nodes) {
                            if (min_depth == -1 || n["level"] < min_depth) {
                                min_depth = n["level"];
                            }
                        }

                        data.nodes.add(ev.nodes);
                        data.edges.add(ev.edges);

                        // Update the position of the button to load more events
                        for (let ev of @{old_earliest_events}) {
                            data.edges.remove("subdag_" + view_id.id + "_" + ev + "_more_ev");
                        }
                        data.nodes.remove("subdag_" + view_id.id + "_more_ev");
                        data.nodes.add({
                            id: "subdag_" + view_id.id + "_more_ev",
                            label: "Load more events",
                            level: min_depth - 1
                        });
                        for (let ev of @{new_earliest_events}) {
                            data.edges.add({
                                id: "subdag_" + view_id.id + "_" + ev + "_move_ev",
                                from: "subdag_" + view_id.id + "_" + ev,
                                to: "subdag_" + view_id.id + "_more_ev"
                            });
                        }

                        return data;
                    });
                }
                BackendChoice::Postgres => {
                    self.data = Some(js! {
                        var view_id = @{view_id};
                        var data = @{data};
                        var ev = @{earlier_events};

                        data.nodes.add(ev.nodes);
                        data.edges.add(ev.edges);

                        for (let ev of @{old_orphan_events}) {
                            data.edges.remove("subdag_" + view_id.id + "_" + ev.id + "_more_of");
                            data.nodes.remove("subdag_" + view_id.id + "_more_of_" + ev.id);
                        }
                        for (let ev of @{new_orphan_events}) {
                            data.nodes.add({
                                id: "subdag_" + view_id.id + "_more_of_" + ev.id,
                                label: "Load ancestors",
                                level: ev.depth - 1
                            });

                            data.edges.add({
                                id: "subdag_" + view_id.id + "_" + ev.id + "_more_of",
                                from: "subdag_" + view_id.id + "_" + ev.id,
                                to: "subdag_" + view_id.id + "_more_of_" + ev.id
                            });
                        }

                        return data;
                    });
                }
            }

            self.earliest_events[view_id.id] = events_dag.earliest_events.clone();
            self.orphan_events[view_id.id] = events_dag.orphan_events.clone();
        }

        if self.latest_events[view_id] != events_dag.latest_events {
            let data = self.data.as_ref().expect("No data set found");

            let mut new_events = DataSet::new();
            events_dag.add_new_events_to_data_set(&mut new_events, self.latest_events[0].clone());
            new_events.add_prefix(&format!("subdag_{}_", view_id));

            self.data = Some(js! {
                var data = @{data};
                var ev = @{new_events};

                data.nodes.add(ev.nodes);
                data.edges.add(ev.edges);

                return data;
            });

            self.latest_events[view_id] = events_dag.latest_events.clone();
        }
    }

    /// Updates the labels of the nodes corresponding to the events in `events_dag` in the view
    /// `view_id`.
    pub fn update_labels(&mut self, events_dag: Arc<RwLock<RoomEvents>>, view_id: usize) {
        self.update_dag(events_dag.clone(), view_id);

        let data = self.data.as_ref().expect("No data set found");
        let events_dag = events_dag.read().unwrap();
        let mut new_data = events_dag.create_data_set();
        new_data.add_prefix(&format!("subdag_{}_", view_id));

        self.data = Some(js! {
            var data = @{data};
            var new_data = @{new_data};

            data.nodes.update(new_data.nodes);

            return data;
        });
    }

    // TODO: maybe this will have to change
    pub fn is_active(&self) -> bool {
        self.network.is_some()
    }
}
