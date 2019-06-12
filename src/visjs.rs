use std::sync::{Arc, RwLock};

use crate::model::dag::DataSet;
use stdweb::web;
use stdweb::web::IParentNode;
use stdweb::Value;

use crate::model::dag::RoomEvents;

#[derive(Default)]
pub struct VisJsService {
    lib: Option<Value>,
    network: Option<Value>,
    data: Option<Value>,
    earliest_events: Vec<String>,
    latest_events: Vec<String>,
}

impl VisJsService {
    pub fn new() -> Self {
        let lib = js! {
            return vis;
        };

        VisJsService {
            lib: Some(lib),
            network: None,
            data: None,
            earliest_events: Vec::new(),
            latest_events: Vec::new(),
        }
    }

    pub fn display_dag(
        &mut self,
        events_dag: Arc<RwLock<RoomEvents>>,
        container_id: &str,
        more_ev_btn_id: &str,
        selected_event_input_id: &str,
        display_body_btn_id: &str,
    ) {
        let lib = self.lib.as_ref().expect("vis library object lost");
        let mut events_dag = events_dag.write().unwrap();

        let data = events_dag.create_data_set();
        let earliest_events = &events_dag.earliest_events;

        let container = web::document()
            .query_selector(container_id)
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

        js_serializable!(DataSet);

        self.data = Some(js! {
            var d = @{data};

            var min_depth = -1;
            for (let n of d.nodes) {
                if (min_depth == -1 || n["level"] < min_depth) {
                    min_depth = n["level"];
                }
            }

            var nodes = new vis.DataSet(d.nodes);
            var edges = new vis.DataSet(d.edges);

            // Add special button to load more events
            nodes.add({
                id: "more_ev",
                label: "Load more events",
                level: min_depth - 1
            });
            for (let ev of @{earliest_events}) {
                edges.add({
                    id: ev,
                    from: ev,
                    to: "more_ev"
                });
            }

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
                    arrows: "to"
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

                if (id == "more_ev") {
                    @{more_ev_btn}.click();
                }
            }

            network.on("selectNode", select_node);

            function display_json_body(ev) {
                let id_input = @{selected_event_input};

                id_input.value = ev.nodes[0];
                @{display_body_btn}.click();
            }

            network.on("doubleClick", display_json_body);

            return network;
        });

        self.earliest_events = events_dag.earliest_events.clone();
        self.latest_events = events_dag.latest_events.clone();
    }

    pub fn update_dag(&mut self, events_dag: Arc<RwLock<RoomEvents>>) {
        let events_dag = events_dag.read().unwrap();

        if self.earliest_events != events_dag.earliest_events {
            let old_earliest_events = self.earliest_events.clone();
            let new_earliest_events = events_dag.earliest_events.clone();

            let data = self.data.as_ref().expect("No data set found");

            let mut earlier_events = DataSet::new();
            events_dag
                .add_earlier_events_to_data_set(&mut earlier_events, old_earliest_events.clone());

            self.data = Some(js! {
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
                    data.edges.remove(ev);
                }
                data.nodes.remove("more_ev");
                data.nodes.add({
                    id: "more_ev",
                    label: "Load more events",
                    level: min_depth - 1
                });
                for (let ev of @{new_earliest_events}) {
                    data.edges.add({
                        id: ev,
                        from: ev,
                        to: "more_ev"
                    });
                }

                return data;
            });

            self.earliest_events = events_dag.earliest_events.clone();
        }

        if self.latest_events != events_dag.latest_events {
            let data = self.data.as_ref().expect("No data set found");

            let mut new_events = DataSet::new();
            events_dag.add_new_events_to_data_set(&mut new_events, self.latest_events.clone());

            self.data = Some(js! {
                var data = @{data};
                var ev = @{new_events};

                data.nodes.add(ev.nodes);
                data.edges.add(ev.edges);

                return data;
            });

            self.latest_events = events_dag.latest_events.clone();
        }
    }
}
