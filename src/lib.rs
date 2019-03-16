extern crate petgraph;
extern crate yew;

mod cs_backend;
mod model;

use yew::services::ConsoleService;
use yew::{html, Component, ComponentLink, Html, Renderable, ShouldRender};

use cs_backend::session::Session;

pub struct Model {
    console: ConsoleService,
    session: Session,
}

pub enum Msg {
    ServerName(html::ChangeData),
    Username(html::ChangeData),
    Password(html::ChangeData),
    RoomId(html::ChangeData),
    Connect,
    Disconnect,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, _: ComponentLink<Self>) -> Self {
        Model {
            console: ConsoleService::new(),
            session: Session::empty(),
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::ServerName(sn) => {
                if let html::ChangeData::Value(sn) = sn {
                    self.session.server_name = sn;
                }
            }
            Msg::Username(u) => {
                if let html::ChangeData::Value(u) = u {
                    self.session.username = u;
                }
            }
            Msg::Password(p) => {
                if let html::ChangeData::Value(p) = p {
                    self.session.password = p;
                }
            }
            Msg::RoomId(ri) => {
                if let html::ChangeData::Value(ri) = ri {
                    self.session.room_id = ri;
                }
            }
            Msg::Connect => {
                self.session.user_id =
                    format!("@{}:{}", self.session.username, self.session.server_name);

                self.console.log(&format!("Info: {:?}", self.session));
            }
            Msg::Disconnect => {
                self.console.log("Disconnecting");
            }
        }

        true
    }
}

impl Renderable<Model> for Model {
    fn view(&self) -> Html<Self> {
        html! {
            <ul>
                <li>{ "Server name: " }<input type="text", onchange=|e| Msg::ServerName(e),/></li>

                <li>{ "Username: " }<input type="text", onchange=|e| Msg::Username(e),/></li>

                <li>{ "Password: " }<input type="password", onchange=|e| Msg::Password(e),/></li>

                <li>{ "Room ID: " }<input type="text", onchange=|e| Msg::RoomId(e),/></li>

                <li><button onclick=|_| Msg::Connect,>{ "Connect" }</button></li>

                <li><button onclick=|_| Msg::Disconnect,>{ "Disconnect" }</button></li>
            </ul>
        }
    }
}
