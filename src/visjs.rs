use stdweb::web;
use stdweb::web::IParentNode;
use stdweb::Value;

use crate::model::dag::RoomEvents;

#[derive(Default)]
pub struct VisJsService(Option<Value>);

impl VisJsService {
    pub fn new() -> Self {
        let lib = js! {
            return vis;
        };

        VisJsService(Some(lib))
    }

    pub fn display_dag(&self, events_dag: &RoomEvents, id: &str) {
        let lib = self.0.as_ref().expect("vis library object lost");

        let dot_string = events_dag.to_dot();

        let container = web::document()
            .query_selector(id)
            .expect("Couldnt get document element")
            .expect("Couldn't get document element");

        js! { @(no_return)
            var vis = @{lib};

            var parsedData = vis.network.convertDot(@{dot_string});

            var data = {
                nodes: parsedData.nodes,
                edges: parsedData.edges
            };

            var options = parsedData.options;
            options.edges = {
                length: 500
            };

            var network = new vis.Network(@{container}, data, options);
        };
    }
}
