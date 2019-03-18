extern crate failure;
extern crate petgraph;
extern crate serde_derive;
extern crate yew;

mod cs_backend;
mod model;

use failure::Error;
use yew::services::fetch::FetchTask;
use yew::services::{ConsoleService, FetchService};
use yew::{html, Callback, Component, ComponentLink, Html, Renderable, ShouldRender};

use cs_backend::authentication::ConnectionResponse;
use cs_backend::session::Session;

pub struct Model {
    console: ConsoleService,
    fetch: FetchService,

    connection_callback: Callback<Result<ConnectionResponse, Error>>,
    connection_task: Option<FetchTask>,

    disconnection_callback: Callback<Result<(), Error>>,
    disconnection_task: Option<FetchTask>,

    session: Session,
}

pub enum Msg {
    ServerName(html::ChangeData),
    RoomId(html::ChangeData),

    Username(html::ChangeData),
    Password(html::ChangeData),

    Connect,
    Disconnect,

    Connected(cs_backend::authentication::ConnectionResponse),
    ConnectionFailed,

    Disconnected,
    DisconnectionFailed,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, mut link: ComponentLink<Self>) -> Self {
        Model {
            console: ConsoleService::new(),
            fetch: FetchService::new(),

            connection_callback: link.send_back(|response: Result<ConnectionResponse, Error>| {
                match response {
                    Ok(res) => Msg::Connected(res),
                    Err(_) => Msg::ConnectionFailed,
                }
            }),
            connection_task: None,

            disconnection_callback: link.send_back(|response: Result<(), Error>| {
                match response {
                    Ok(_) => Msg::Disconnected,
                    Err(_) => Msg::DisconnectionFailed,
                }
            }),
            disconnection_task: None,

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
            Msg::RoomId(ri) => {
                if let html::ChangeData::Value(ri) = ri {
                    self.session.room_id = ri;
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
            Msg::Connect => {
                let task = self.connect(self.connection_callback.clone());
                self.connection_task = Some(task);

                self.console.log(&format!("Info: {:?}", self.session));
            }
            Msg::Disconnect => {
                self.console.log("Disconnecting");

                let task = self.disconnect(self.disconnection_callback.clone());
                self.disconnection_task = Some(task);
            }
            Msg::Connected(res) => {
                self.session.user_id = res.user_id;
                self.session.access_token = Some(res.access_token);
                self.session.device_id = Some(res.device_id);

                self.console.log(&format!(
                    "Connected with token: {} and as {}",
                    self.session.access_token.as_ref().unwrap(),
                    self.session.device_id.as_ref().unwrap()
                ));

                self.connection_task = None;
            }
            Msg::ConnectionFailed => {
                self.console.log("Connection failed");

                self.connection_task = None;
            }
            Msg::Disconnected => {
                self.console.log("Disconnected");

                self.disconnection_task = None;
            }
            Msg::DisconnectionFailed => {
                self.console.log("Could not disconnect");

                self.disconnection_task = None;
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

                <li>{ "Room ID: " }<input type="text", onchange=|e| Msg::RoomId(e),/></li>

                <li>{ "Username: " }<input type="text", onchange=|e| Msg::Username(e),/></li>

                <li>{ "Password: " }<input type="password", onchange=|e| Msg::Password(e),/></li>

                <li><button onclick=|_| Msg::Connect,>{ "Connect" }</button></li>

                <li><button onclick=|_| Msg::Disconnect,>{ "Disconnect" }</button></li>
            </ul>
        }
    }
}
