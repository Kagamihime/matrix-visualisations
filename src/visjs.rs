use crate::model::dag::DataSet;
use stdweb::web;
use stdweb::web::IParentNode;
use stdweb::Value;

use crate::model::dag::RoomEvents;

#[derive(Default)]
pub struct VisJsService {
    lib: Option<Value>,
    data: Option<Value>,
    earliest_event: String,
    latest_event: String,
}

impl VisJsService {
    pub fn new() -> Self {
        let lib = js! {
            return vis;
        };

        VisJsService {
            lib: Some(lib),
            data: None,
            earliest_event: String::new(),
            latest_event: String::new(),
        }
    }

    pub fn display_dag(
        &mut self,
        events_dag: &mut RoomEvents,
        container_id: &str,
        more_ev_btn_id: &str,
    ) {
        let lib = self.lib.as_ref().expect("vis library object lost");

        let data = events_dag.create_data_set();
        let earliest_event = &events_dag.earliest_event;

        let container = web::document()
            .query_selector(container_id)
            .expect("Couldn't get document element")
            .expect("Couldn't get document element");
        let more_ev_btn = web::document()
            .query_selector(more_ev_btn_id)
            .expect("Couldn't get document element")
            .expect("Couldn't get document element");

        js_serializable!(DataSet);

        self.data = Some(js! {
            var vis = @{lib};

            var d = @{data};

            var nodes = new vis.DataSet(d.nodes);
            var edges = new vis.DataSet(d.edges);

            // Add special button to load more events
            nodes.add({
                id: "more_ev",
                label: "Load more events"
            });
            edges.add({
                id: "more_ev_edge",
                from: @{earliest_event},
                to: "more_ev"
            });

            var data = {
                nodes: nodes,
                edges: edges
            };

            var options = {
                layout: {
                    randomSeed: undefined,
                    improvedLayout: true,
                    hierarchical: {
                        enabled: true,
                        levelSeparation: 200,
                        nodeSpacing: 300,
                        direction: "DU",
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

            function load_more() {
                @{more_ev_btn}.click();
            }

            network.on("selectNode", load_more);

            return data;
        });

        self.earliest_event = events_dag.earliest_event.clone();
        self.latest_event = events_dag.latest_event.clone();
    }

    pub fn update_dag(&mut self, events_dag: &RoomEvents) {
        if self.earliest_event != events_dag.earliest_event {
            let data = self.data.as_ref().expect("No data set found");
            let earliest_event = &events_dag.earliest_event;
            let earlier_events = events_dag.get_earlier_events(self.earliest_event.clone());

            self.data = Some(js! {
                var data = @{data};
                var ev = @{earlier_events};

                data.nodes.add(ev.nodes);
                data.edges.add(ev.edges);

                // Update the position of the button to load more events
                data.edges.remove("more_ev_edge");
                data.nodes.remove("more_ev");
                data.nodes.add({
                    id: "more_ev",
                    label: "Load more events"
                });
                data.edges.add({
                    id: "more_ev_edge",
                    from: @{earliest_event},
                    to: "more_ev"
                });

                return data;
            });

            self.earliest_event = events_dag.earliest_event.clone();
        }

        if self.latest_event != events_dag.latest_event {
            let data = self.data.as_ref().expect("No data set found");
            let new_events = events_dag.get_new_events(self.latest_event.clone());

            self.data = Some(js! {
                var data = @{data};
                var ev = @{new_events};

                data.nodes.add(ev.nodes);
                data.edges.add(ev.edges);

                return data;
            });

            self.latest_event = events_dag.latest_event.clone();
        }
    }
}
