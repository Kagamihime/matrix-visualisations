#![recursion_limit = "128"]

extern crate failure;
extern crate petgraph;
extern crate serde_derive;
extern crate serde_json;
#[macro_use]
extern crate stdweb;
extern crate yew;

mod cs_backend;
mod model;
mod visjs;

use failure::Error;
use yew::services::fetch::FetchTask;
use yew::services::{ConsoleService, FetchService};
use yew::{html, Callback, Component, ComponentLink, Html, Renderable, ShouldRender};

use cs_backend::authentication::ConnectionResponse;
use cs_backend::events::SyncResponse;
use cs_backend::rooms::JoinedRooms;
use cs_backend::session::Session;
use model::dag::RoomEvents;
use visjs::VisJsService;

pub struct Model {
    console: ConsoleService,
    fetch: FetchService,
    vis: VisJsService,
    link: ComponentLink<Self>,

    connection_callback: Callback<Result<ConnectionResponse, Error>>,
    connection_task: Option<FetchTask>,

    listing_rooms_callback: Callback<Result<JoinedRooms, Error>>,
    listing_rooms_task: Option<FetchTask>,

    joining_room_callback: Callback<Result<(), Error>>,
    joining_room_task: Option<FetchTask>,

    sync_callback: Callback<Result<SyncResponse, Error>>,
    sync_task: Option<FetchTask>,

    disconnection_callback: Callback<Result<(), Error>>,
    disconnection_task: Option<FetchTask>,

    session: Session,
    events_dag: Option<RoomEvents>,
}

pub enum Msg {
    ServerName(html::ChangeData),
    RoomId(html::ChangeData),

    Username(html::ChangeData),
    Password(html::ChangeData),

    Connect,
    ListRooms,
    JoinRoom,
    Sync,
    Disconnect,

    Connected(ConnectionResponse),
    ConnectionFailed,

    ListedRooms(JoinedRooms),
    ListingRoomsFailed,

    RoomJoined,
    RoomJoiningFailed,

    Synced(SyncResponse),
    SyncFailed,

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
            vis: VisJsService::new(),

            connection_callback: link.send_back(|response: Result<ConnectionResponse, Error>| {
                match response {
                    Ok(res) => Msg::Connected(res),
                    Err(_) => Msg::ConnectionFailed,
                }
            }),
            connection_task: None,

            listing_rooms_callback: link.send_back(|response: Result<JoinedRooms, Error>| {
                match response {
                    Ok(res) => Msg::ListedRooms(res),
                    Err(e) => {
                        ConsoleService::new().log(&format!("{}", e));
                        Msg::ListingRoomsFailed
                    }
                }
            }),
            listing_rooms_task: None,

            joining_room_callback: link.send_back(|response: Result<(), Error>| match response {
                Ok(_) => Msg::RoomJoined,
                Err(_) => Msg::RoomJoiningFailed,
            }),
            joining_room_task: None,

            sync_callback: link.send_back(|response: Result<SyncResponse, Error>| match response {
                Ok(res) => Msg::Synced(res),
                Err(_) => Msg::SyncFailed,
            }),
            sync_task: None,

            disconnection_callback: link.send_back(|response: Result<(), Error>| match response {
                Ok(_) => Msg::Disconnected,
                Err(_) => Msg::DisconnectionFailed,
            }),
            disconnection_task: None,

            link,

            session: Session::empty(),
            events_dag: None,
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
                self.console.log("Connecting...");

                let task = self.connect(self.connection_callback.clone());
                self.connection_task = Some(task);
            }
            Msg::ListRooms => {
                self.console.log("Listing joined rooms...");

                let task = self.list_rooms(self.listing_rooms_callback.clone());
                self.listing_rooms_task = Some(task);
            }
            Msg::JoinRoom => {
                self.console.log("Joining the room...");

                let task = self.join_room(self.joining_room_callback.clone());
                self.joining_room_task = Some(task);
            }
            Msg::Sync => {
                self.console.log("Syncing...");

                let task = self.sync(self.sync_callback.clone());
                self.sync_task = Some(task);
            }
            Msg::Disconnect => {
                self.console.log("Disconnecting...");

                match self.session.access_token {
                    None => {
                        self.console.log("You were not connected");
                    }
                    Some(_) => {
                        let task = self.disconnect(self.disconnection_callback.clone());
                        self.disconnection_task = Some(task);
                    }
                }
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

                self.link.send_back(|_: ()| Msg::ListRooms).emit(());
            }
            Msg::ConnectionFailed => {
                self.console.log("Connection failed");

                self.connection_task = None;
            }
            Msg::ListedRooms(res) => {
                self.console.log("Looking up in joined rooms");

                self.listing_rooms_task = None;

                if res.joined_rooms.contains(&self.session.room_id) {
                    self.link.send_back(|_: ()| Msg::Sync).emit(());
                } else {
                    self.link.send_back(|_: ()| Msg::JoinRoom).emit(());
                }
            }
            Msg::ListingRoomsFailed => {
                self.console.log("Failed to get the list of joined rooms");

                self.listing_rooms_task = None;
            }
            Msg::RoomJoined => {
                self.console.log("Room joined!");

                self.joining_room_task = None;

                self.link.send_back(|_: ()| Msg::Sync).emit(());
            }
            Msg::RoomJoiningFailed => {
                self.console.log("Failed to join the room");

                self.joining_room_task = None;
            }
            Msg::Synced(res) => {
                self.events_dag = model::dag::RoomEvents::from_sync_response(
                    &self.session.room_id,
                    &self.session.server_name,
                    res,
                );

                match &self.events_dag {
                    Some(dag) => {
                        self.console.log("Events DAG built!");
                        self.console.log(&dag.to_dot());

                        self.vis.display_dag(dag, "#dag-vis");
                    }
                    None => self.console.log("Failed to build the DAG"),
                }

                self.sync_task = None;
            }
            Msg::SyncFailed => {
                self.console.log("Could not sync");

                self.sync_task = None;
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

                <li>
                    <button onclick=|_| Msg::Connect,>{ "Connect" }</button>
                    <button onclick=|_| Msg::Disconnect,>{ "Disconnect" }</button>
                </li>
            </ul>

            <section id="dag-vis",>
            </section>
        }
    }
}
