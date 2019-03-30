use stdweb::web;
use stdweb::web::IParentNode;
use stdweb::Value;

#[derive(Default)]
pub struct VisJsService(Option<Value>);

impl VisJsService {
    pub fn new() -> Self {
        let lib = js! {
            return vis;
        };

        VisJsService(Some(lib))
    }

    pub fn display_dag(&self, id: &str) {
        let lib = self.0.as_ref().expect("vis library object lost");

        let container = web::document()
            .query_selector(id)
            .expect("Couldnt get document element")
            .expect("Couldn't get document element");

        js! { @(no_return)
            var vis = @{lib};

            var DOTstring = "dinetwork {1 -> 1 -> 2; 2 -> 3; 2 -- 4; 2 -> 1 }";
            var parsedData = vis.network.convertDot(DOTstring);

            var data = {
                nodes: parsedData.nodes,
                edges: parsedData.edges
            };

            var options = parsedData.options;

            var network = new vis.Network(@{container}, data, options);
        };
    }
}
