#![recursion_limit = "128"]

extern crate failure;
extern crate percent_encoding;
extern crate petgraph;
extern crate serde_derive;
extern crate serde_json;
#[macro_use]
extern crate stdweb;
extern crate yew;

mod cs_backend;
mod model;
mod visjs;

use std::sync::{Arc, RwLock};

use failure::Error;
use yew::services::fetch::FetchTask;
use yew::services::ConsoleService;
use yew::{html, Callback, Component, ComponentLink, Html, Renderable, ShouldRender};

use cs_backend::backend::{
    CSBackend, ConnectionResponse, JoinedRooms, MessagesResponse, SyncResponse,
};
use cs_backend::session::Session;
use model::dag::RoomEvents;
use visjs::VisJsService;

pub struct Model {
    console: ConsoleService,
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

    more_msg_callback: Callback<Result<MessagesResponse, Error>>,
    more_msg_task: Option<FetchTask>,

    disconnection_callback: Callback<Result<(), Error>>,
    disconnection_task: Option<FetchTask>,

    cs_backend: CSBackend,
    session: Arc<RwLock<Session>>,
    events_dag: Option<RoomEvents>,
}

pub enum Msg {
    UI(UIEvent),
    BkCmd(BkCommand),
    BkRes(BkResponse),
}

pub enum UIEvent {
    ServerName(html::ChangeData),
    RoomId(html::ChangeData),

    Username(html::ChangeData),
    Password(html::ChangeData),
}

pub enum BkCommand {
    Connect,
    ListRooms,
    JoinRoom,
    Sync,
    MoreMsg,
    Disconnect,
}

pub enum BkResponse {
    Connected(ConnectionResponse),
    RoomsList(JoinedRooms),
    RoomJoined,
    Synced(SyncResponse),
    MsgGot(MessagesResponse),
    Disconnected,

    ConnectionFailed,
    ListingRoomsFailed,
    JoiningRoomFailed,
    SyncFailed,
    MoreMsgFailed,
    DisconnectionFailed,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, mut link: ComponentLink<Self>) -> Self {
        let new_session = Arc::new(RwLock::new(Session::empty()));

        Model {
            console: ConsoleService::new(),
            vis: VisJsService::new(),

            connection_callback: link.send_back(|response: Result<ConnectionResponse, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::Connected(res)),
                    Err(_) => Msg::BkRes(BkResponse::ConnectionFailed),
                }
            }),
            connection_task: None,

            listing_rooms_callback: link.send_back(|response: Result<JoinedRooms, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::RoomsList(res)),
                    Err(e) => {
                        ConsoleService::new().log(&format!("{}", e));
                        Msg::BkRes(BkResponse::ListingRoomsFailed)
                    }
                }
            }),
            listing_rooms_task: None,

            joining_room_callback: link.send_back(|response: Result<(), Error>| match response {
                Ok(_) => Msg::BkRes(BkResponse::RoomJoined),
                Err(_) => Msg::BkRes(BkResponse::JoiningRoomFailed),
            }),
            joining_room_task: None,

            sync_callback: link.send_back(|response: Result<SyncResponse, Error>| match response {
                Ok(res) => Msg::BkRes(BkResponse::Synced(res)),
                Err(_) => Msg::BkRes(BkResponse::SyncFailed),
            }),
            sync_task: None,

            more_msg_callback: link.send_back(|response: Result<MessagesResponse, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::MsgGot(res)),
                    Err(_) => Msg::BkRes(BkResponse::MoreMsgFailed),
                }
            }),
            more_msg_task: None,

            disconnection_callback: link.send_back(|response: Result<(), Error>| match response {
                Ok(_) => Msg::BkRes(BkResponse::Disconnected),
                Err(_) => Msg::BkRes(BkResponse::DisconnectionFailed),
            }),
            disconnection_task: None,

            link,

            cs_backend: CSBackend::with_session(new_session.clone()),
            session: new_session,
            events_dag: None,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::UI(ui) => self.process_ui_event(ui),
            Msg::BkCmd(cmd) => self.process_bk_command(cmd),
            Msg::BkRes(res) => self.process_bk_response(res),
        }

        true
    }
}

impl Model {
    fn process_ui_event(&mut self, event: UIEvent) {
        match event {
            UIEvent::ServerName(sn) => {
                if let html::ChangeData::Value(sn) = sn {
                    self.session.write().unwrap().server_name = sn;
                }
            }
            UIEvent::RoomId(ri) => {
                if let html::ChangeData::Value(ri) = ri {
                    self.session.write().unwrap().room_id = ri;
                }
            }
            UIEvent::Username(u) => {
                if let html::ChangeData::Value(u) = u {
                    self.session.write().unwrap().username = u;
                }
            }
            UIEvent::Password(p) => {
                if let html::ChangeData::Value(p) = p {
                    self.session.write().unwrap().password = p;
                }
            }
        }
    }

    fn process_bk_command(&mut self, cmd: BkCommand) {
        let console_msg = match cmd {
            BkCommand::Connect => "Connecting...",
            BkCommand::ListRooms => "Listing joined rooms...",
            BkCommand::JoinRoom => "Joining the room...",
            BkCommand::Sync => "Syncing...",
            BkCommand::MoreMsg => "Retrieving previous messages...",
            BkCommand::Disconnect => "Disconnecting...",
        };

        self.console.log(console_msg);

        match cmd {
            BkCommand::Connect => {
                self.connection_task =
                    Some(self.cs_backend.connect(self.connection_callback.clone()))
            }
            BkCommand::ListRooms => {
                self.listing_rooms_task = Some(
                    self.cs_backend
                        .list_rooms(self.listing_rooms_callback.clone()),
                )
            }
            BkCommand::JoinRoom => {
                self.joining_room_task = Some(
                    self.cs_backend
                        .join_room(self.joining_room_callback.clone()),
                )
            }
            BkCommand::Sync => {
                self.sync_task = Some(self.cs_backend.sync(self.sync_callback.clone()))
            }
            BkCommand::MoreMsg => {
                self.more_msg_task = Some(
                    self.cs_backend
                        .get_prev_messages(self.more_msg_callback.clone()),
                )
            }
            BkCommand::Disconnect => match self.session.read().unwrap().access_token {
                None => {
                    self.console.log("You were not connected");
                }
                Some(_) => {
                    self.disconnection_task = Some(
                        self.cs_backend
                            .disconnect(self.disconnection_callback.clone()),
                    );
                }
            },
        }
    }

    fn process_bk_response(&mut self, res: BkResponse) {
        match res {
            BkResponse::Connected(res) => {
                self.connection_task = None;

                let mut session = self.session.write().unwrap();

                session.user_id = res.user_id;
                session.access_token = Some(res.access_token);
                session.device_id = Some(res.device_id);

                self.console.log(&format!(
                    "Connected with token: {} and as {}",
                    session.access_token.as_ref().unwrap(),
                    session.device_id.as_ref().unwrap()
                ));

                self.link
                    .send_back(|_: ()| Msg::BkCmd(BkCommand::ListRooms))
                    .emit(());
            }
            BkResponse::RoomsList(res) => {
                self.console.log("Looking up in joined rooms");

                self.listing_rooms_task = None;

                if res
                    .joined_rooms
                    .contains(&self.session.read().unwrap().room_id)
                {
                    self.link
                        .send_back(|_: ()| Msg::BkCmd(BkCommand::Sync))
                        .emit(());
                } else {
                    self.link
                        .send_back(|_: ()| Msg::BkCmd(BkCommand::JoinRoom))
                        .emit(());
                }
            }
            BkResponse::RoomJoined => {
                self.console.log("Room joined!");

                self.joining_room_task = None;

                self.link
                    .send_back(|_: ()| Msg::BkCmd(BkCommand::Sync))
                    .emit(());
            }
            BkResponse::Synced(res) => {
                self.sync_task = None;

                let mut session = self.session.write().unwrap();

                session.next_batch_token = Some(res.next_batch.clone());

                if session.prev_batch_token.is_none() {
                    if let Some(room) = res.rooms.join.get(&session.room_id) {
                        session.prev_batch_token = room.timeline.prev_batch.clone();
                    }
                }

                self.events_dag = model::dag::RoomEvents::from_sync_response(
                    &session.room_id,
                    &session.server_name,
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

                // TODO: do not call this right after
                self.link
                    .send_back(|_: ()| Msg::BkCmd(BkCommand::MoreMsg))
                    .emit(());
            }
            BkResponse::MsgGot(res) => {
                self.more_msg_task = None;

                self.session.write().unwrap().prev_batch_token = Some(res.end);

                self.console.log("Previous messages:");

                for msg in res.chunk {
                    if let Ok(msg) = serde_json::to_string_pretty(&msg) {
                        self.console.log(&msg);
                    }
                }

                // TODO: add events to the DAG
            }
            BkResponse::Disconnected => {
                self.console.log("Disconnected");

                self.disconnection_task = None;
            }
            BkResponse::ConnectionFailed => {
                self.console.log("Connection failed");

                self.connection_task = None;
            }
            BkResponse::ListingRoomsFailed => {
                self.console.log("Failed to get the list of joined rooms");

                self.listing_rooms_task = None;
            }
            BkResponse::JoiningRoomFailed => {
                self.console.log("Failed to join the room");

                self.joining_room_task = None;
            }
            BkResponse::SyncFailed => {
                self.console.log("Could not sync");

                self.sync_task = None;
            }
            BkResponse::MoreMsgFailed => {
                self.console.log("Could not retrieve previous messages");

                self.more_msg_task = None;
            }
            BkResponse::DisconnectionFailed => {
                self.console.log("Could not disconnect");

                self.disconnection_task = None;
            }
        }
    }
}

impl Renderable<Model> for Model {
    fn view(&self) -> Html<Self> {
        html! {
            <ul>
                <li>{ "Server name: " }<input type="text", onchange=|e| Msg::UI(UIEvent::ServerName(e)),/></li>

                <li>{ "Room ID: " }<input type="text", onchange=|e| Msg::UI(UIEvent::RoomId(e)),/></li>

                <li>{ "Username: " }<input type="text", onchange=|e| Msg::UI(UIEvent::Username(e)),/></li>

                <li>{ "Password: " }<input type="password", onchange=|e| Msg::UI(UIEvent::Password(e)),/></li>

                <li>
                    <button onclick=|_| Msg::BkCmd(BkCommand::Connect),>{ "Connect" }</button>
                    <button onclick=|_| Msg::BkCmd(BkCommand::Disconnect),>{ "Disconnect" }</button>
                </li>
            </ul>

            <section id="dag-vis",>
            </section>
        }
    }
}
