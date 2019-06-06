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

    pub fn display_dag(&mut self, events_dag: &mut RoomEvents, id: &str) {
        let lib = self.lib.as_ref().expect("vis library object lost");

        let data = events_dag.create_data_set();

        let container = web::document()
            .query_selector(id)
            .expect("Couldnt get document element")
            .expect("Couldn't get document element");

        js_serializable!(DataSet);

        self.data = Some(js! {
            var vis = @{lib};

            var d = @{data};

            var nodes = new vis.DataSet(d.nodes);
            var edges = new vis.DataSet(d.edges);

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
                        minimum: 150,
                        maximum: 200
                    }
                }
            };

            var network = new vis.Network(@{container}, data, options);

            return data;
        });

        self.earliest_event = events_dag.earliest_event.clone();
        self.latest_event = events_dag.latest_event.clone();
    }

    pub fn update_dag(&mut self, events_dag: &RoomEvents) {
        if self.earliest_event != events_dag.earliest_event {
            let data = self.data.as_ref().expect("No data set found");
            let earlier_events = events_dag.get_earlier_events(self.earliest_event.clone());

            self.data = Some(js! {
                var data = @{data};
                var ev = @{earlier_events};

                data.nodes.add(ev.nodes);
                data.edges.add(ev.edges);

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
