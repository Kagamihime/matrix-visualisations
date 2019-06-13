#![recursion_limit = "512"]

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
use stdweb::unstable::TryInto;
use stdweb::web;
use stdweb::web::IParentNode;
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

    leaving_room_callback: Callback<Result<(), Error>>,
    leaving_room_task: Option<FetchTask>,

    disconnection_callback: Callback<Result<(), Error>>,
    disconnection_task: Option<FetchTask>,

    cs_backend: CSBackend,
    session: Arc<RwLock<Session>>,
    events_dag: Option<Arc<RwLock<RoomEvents>>>,
    event_body: Option<String>,
}

pub enum Msg {
    UI(UIEvent),
    UICmd(UICommand),
    BkCmd(BkCommand),
    BkRes(BkResponse),
}

/// These messages notifies the application of changes in the data modifiable via the UI.
pub enum UIEvent {
    ServerName(html::ChangeData),
    RoomId(html::ChangeData),

    Username(html::ChangeData),
    Password(html::ChangeData),
}

pub enum UICommand {
    DisplayEventBody,
}

/// These messages are used by the frontend to send commands to the backend.
pub enum BkCommand {
    Connect,
    ListRooms,
    JoinRoom,
    Sync,
    MoreMsg,
    LeaveRoom,
    Disconnect,
}

/// These messages are responses from the backend to the frontend.
pub enum BkResponse {
    Connected(ConnectionResponse),
    RoomsList(JoinedRooms),
    RoomJoined,
    Synced(SyncResponse),
    MsgGot(MessagesResponse),
    RoomLeft,
    Disconnected,

    ConnectionFailed,
    ListingRoomsFailed,
    JoiningRoomFailed,
    SyncFailed,
    MoreMsgFailed,
    LeavingRoomFailed,
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

            leaving_room_callback: link.send_back(|response: Result<(), Error>| match response {
                Ok(_) => Msg::BkRes(BkResponse::RoomLeft),
                Err(_) => Msg::BkRes(BkResponse::LeavingRoomFailed),
            }),
            leaving_room_task: None,

            disconnection_callback: link.send_back(|response: Result<(), Error>| match response {
                Ok(_) => Msg::BkRes(BkResponse::Disconnected),
                Err(_) => Msg::BkRes(BkResponse::DisconnectionFailed),
            }),
            disconnection_task: None,

            link,

            cs_backend: CSBackend::with_session(new_session.clone()),
            session: new_session,
            events_dag: None,
            event_body: None,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::UI(ui) => self.process_ui_event(ui),
            Msg::UICmd(cmd) => self.process_ui_command(cmd),
            Msg::BkCmd(cmd) => self.process_bk_command(cmd),
            Msg::BkRes(res) => self.process_bk_response(res),
        }

        true
    }
}

impl Model {
    fn process_ui_event(&mut self, event: UIEvent) {
        // Change the informations of the session whenever their corresponding entries in the UI
        // are changed
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

    fn process_ui_command(&mut self, cmd: UICommand) {
        match cmd {
            UICommand::DisplayEventBody => {
                let input: web::html_element::InputElement = web::document()
                    .query_selector("#selected-event")
                    .expect("Couldn't get document element")
                    .expect("Couldn't get document element")
                    .try_into()
                    .unwrap();
                let event_id = input.raw_value();

                if let Some(dag) = &self.events_dag {
                    self.event_body = dag
                        .read()
                        .unwrap()
                        .get_event(&event_id)
                        .map(|ev| serde_json::to_string_pretty(ev).unwrap());
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
            BkCommand::LeaveRoom => "Leaving the room...",
            BkCommand::Disconnect => "Disconnecting...",
        };

        self.console.log(console_msg);

        // Order the backend to make requests to the homeserver according to the command received
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
                let next_batch_token = self.session.read().unwrap().next_batch_token.clone();

                self.sync_task = Some(
                    self.cs_backend
                        .sync(self.sync_callback.clone(), next_batch_token),
                )
            }
            BkCommand::MoreMsg => {
                self.more_msg_task = Some(
                    self.cs_backend
                        .get_prev_messages(self.more_msg_callback.clone()),
                )
            }
            BkCommand::LeaveRoom => {
                self.leaving_room_task = Some(
                    self.cs_backend
                        .leave_room(self.leaving_room_callback.clone()),
                );
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

                // Save the informations given by the homeserver when connecting to it. The access
                // token will be used for authenticating subsequent requests.
                session.user_id = res.user_id;
                session.access_token = Some(res.access_token);
                session.device_id = Some(res.device_id);

                self.console.log(&format!(
                    "Connected with token: {} and as {}",
                    session.access_token.as_ref().unwrap(),
                    session.device_id.as_ref().unwrap()
                ));

                // Request the list of the rooms joined by the user as soon as we are connected
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
                    // If the user is already in the room to observe, make the initial sync
                    self.link
                        .send_back(|_: ()| Msg::BkCmd(BkCommand::Sync))
                        .emit(());
                } else {
                    // Join the room if the user is not already in it
                    self.link
                        .send_back(|_: ()| Msg::BkCmd(BkCommand::JoinRoom))
                        .emit(());
                }
            }
            BkResponse::RoomJoined => {
                self.console.log("Room joined!");

                self.joining_room_task = None;

                // Make the initial sync as soon as the user has joined the room
                self.link
                    .send_back(|_: ()| Msg::BkCmd(BkCommand::Sync))
                    .emit(());
            }
            BkResponse::Synced(res) => {
                self.sync_task = None;

                let mut session = self.session.write().unwrap();
                let next_batch_token = res.next_batch.clone(); // Save the next batch token to get new events later

                match session.next_batch_token {
                    None => {
                        // Initialise the prev batch token on the initial sync
                        if let Some(room) = res.rooms.join.get(&session.room_id) {
                            session.prev_batch_token = room.timeline.prev_batch.clone();
                        }

                        // Create a new DAG if it is the initial sync
                        if let Some(dag) = model::dag::RoomEvents::from_sync_response(
                            &session.room_id,
                            &session.server_name,
                            res,
                        ) {
                            self.events_dag = Some(Arc::new(RwLock::new(dag)));;
                        }

                        match self.events_dag.clone() {
                            Some(dag) => {
                                // Display the DAG with VisJs if it has been successfully built
                                self.vis.display_dag(
                                    dag,
                                    "#dag-vis",
                                    "#more-ev-target",
                                    "#selected-event",
                                    "#display-body-target",
                                );
                            }
                            None => self.console.log("Failed to build the DAG"),
                        }
                    }
                    Some(_) => match self.events_dag.clone() {
                        // Add new events to the DAG
                        Some(dag) => {
                            dag.write().unwrap().add_new_events(res);
                            self.vis.update_dag(dag);
                        }
                        None => self.console.log("There is no DAG"),
                    },
                }

                session.next_batch_token = Some(next_batch_token);

                // Request for futur new events
                self.link
                    .send_back(|_: ()| Msg::BkCmd(BkCommand::Sync))
                    .emit(());
            }
            BkResponse::MsgGot(res) => {
                self.more_msg_task = None;

                // Save the prev batch token for the next `/messages` request
                self.session.write().unwrap().prev_batch_token = Some(res.end);

                match self.events_dag.clone() {
                    // Add earlier event to the DAG and display them
                    Some(dag) => {
                        dag.write().unwrap().add_prev_events(res.chunk);

                        self.vis.update_dag(dag);
                    }
                    None => self.console.log("There was no DAG"),
                }
            }
            BkResponse::RoomLeft => {
                self.leaving_room_task = None;

                self.console.log("Room left!");

                // Disconnect as soon as we leave the room
                self.link
                    .send_back(|_: ()| Msg::BkCmd(BkCommand::Disconnect))
                    .emit(());
            }
            BkResponse::Disconnected => {
                self.console.log("Disconnected");

                self.sync_task = None; // If a `/sync` request was in progress, cancel it
                self.disconnection_task = None;

                let mut session = self.session.write().unwrap();

                // Erase the current session data so they won't be erroneously used if the user
                // logs in again
                session.access_token = None;
                session.device_id = None;
                session.filter_id = None;
                session.next_batch_token = None;
                session.prev_batch_token = None;

                self.events_dag = None;
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
            BkResponse::LeavingRoomFailed => {
                self.console.log("Failed to leave the room");

                self.leaving_room_task = None;
            }
            BkResponse::DisconnectionFailed => {
                self.console.log("Could not disconnect");

                self.disconnection_task = None;
            }
        }
    }

    fn display_body(&self) -> Html<Model> {
        match &self.event_body {
            Some(body) => {
                html! {
                    <pre><code>{ body }</code></pre>
                }
            }
            None => {
                html! {
                    <p>{ "No JSON body to show yet" }</p>
                }
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
                    <button onclick=|_| Msg::BkCmd(BkCommand::LeaveRoom),> { "Leave room and disconnect" }</button>
                </li>
            </ul>

            <section class="to-hide",>
                <p>{ "The elements in this section should be hidden" }</p>
                <button id="more-ev-target", onclick=|_| Msg::BkCmd(BkCommand::MoreMsg),>{ "More events" }</button>
                <input type="text", id="selected-event",></input>
                <button id="display-body-target", onclick=|_| Msg::UICmd(UICommand::DisplayEventBody),>{ "Display body" }</button>
            </section>

            <div class="view",>
                <section id="dag-vis",>
                </section>

                <section id="event-body",>
                { self.display_body() }
                </section>
            </div>
        }
    }
}
