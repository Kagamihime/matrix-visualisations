#![recursion_limit = "512"]

extern crate failure;
extern crate percent_encoding;
extern crate petgraph;
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate stdweb;
extern crate yew;

mod cs_backend;
mod model;
mod pg_backend;
mod visjs;

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use failure::Error;
use stdweb::unstable::TryInto;
use stdweb::web;
use stdweb::web::IParentNode;
use yew::services::fetch::FetchTask;
use yew::services::timeout::TimeoutTask;
use yew::services::{ConsoleService, TimeoutService};
use yew::{html, Callback, Component, ComponentLink, Html, Renderable, ShouldRender};

use cs_backend::backend::{
    CSBackend, ConnectionResponse, ContextResponse, JoinedRooms, MessagesResponse, SyncResponse,
};
use cs_backend::session::Session as CSSession;
use model::dag::RoomEvents;
use model::event::Field;
use pg_backend::backend::{EventsResponse, PostgresBackend};
use pg_backend::session::Session as PgSession;
use visjs::VisJsService;

pub type ViewIndex = usize;

pub struct Model {
    console: ConsoleService,
    timeout: TimeoutService,
    vis: VisJsService,
    link: ComponentLink<Self>,

    bk_type: Arc<RwLock<BackendChoice>>,
    view_idx: ViewIndex,
    views: Vec<View>,
    event_body: Option<String>,
    room_state: Option<String>,
    fields_choice: FieldsChoice,
}

pub enum View {
    CS(CSView),
    Postgres(PgView),
}

impl View {
    pub fn get_id(&self) -> ViewIndex {
        match self {
            View::CS(v) => v.id,
            View::Postgres(v) => v.id,
        }
    }

    pub fn get_events_dag(&self) -> &Option<Arc<RwLock<RoomEvents>>> {
        match self {
            View::CS(v) => &v.events_dag,
            View::Postgres(v) => &v.events_dag,
        }
    }
}

// This contains every informations needed for the observation of a room from a given HS by using
// the CS API.
pub struct CSView {
    id: ViewIndex,

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

    state_callback: Callback<Result<ContextResponse, Error>>,
    state_task: Option<FetchTask>,

    leaving_room_callback: Callback<Result<(), Error>>,
    leaving_room_task: Option<FetchTask>,

    disconnection_callback: Callback<Result<(), Error>>,
    disconnection_task: Option<FetchTask>,

    session: Arc<RwLock<CSSession>>,
    backend: CSBackend,
    events_dag: Option<Arc<RwLock<RoomEvents>>>,
}

impl CSView {
    pub fn new(id: ViewIndex, link: &mut ComponentLink<Model>) -> CSView {
        let session = Arc::new(RwLock::new(CSSession::empty()));

        CSView {
            id,

            connection_callback: link.send_back(
                move |response: Result<ConnectionResponse, Error>| match response {
                    Ok(res) => Msg::BkRes(BkResponse::Connected(id, res)),
                    Err(_) => Msg::BkRes(BkResponse::ConnectionFailed(id)),
                },
            ),
            connection_task: None,

            listing_rooms_callback: link.send_back(move |response: Result<JoinedRooms, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::RoomsList(id, res)),
                    Err(e) => {
                        ConsoleService::new().log(&format!("{}", e));
                        Msg::BkRes(BkResponse::ListingRoomsFailed(id))
                    }
                }
            }),
            listing_rooms_task: None,

            joining_room_callback: link.send_back(
                move |response: Result<(), Error>| match response {
                    Ok(_) => Msg::BkRes(BkResponse::RoomJoined(id)),
                    Err(_) => Msg::BkRes(BkResponse::JoiningRoomFailed(id)),
                },
            ),
            joining_room_task: None,

            sync_callback: link.send_back(move |response: Result<SyncResponse, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::Synced(id, res)),
                    Err(_) => Msg::BkRes(BkResponse::SyncFailed(id)),
                }
            }),
            sync_task: None,

            more_msg_callback: link.send_back(move |response: Result<MessagesResponse, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::MsgGot(id, res)),
                    Err(_) => Msg::BkRes(BkResponse::MoreMsgFailed(id)),
                }
            }),
            more_msg_task: None,

            state_callback: link.send_back(move |response: Result<ContextResponse, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::StateFetched(id, res)),
                    Err(_) => Msg::BkRes(BkResponse::FetchStateFailed(id)),
                }
            }),
            state_task: None,

            leaving_room_callback: link.send_back(
                move |response: Result<(), Error>| match response {
                    Ok(_) => Msg::BkRes(BkResponse::RoomLeft(id)),
                    Err(_) => Msg::BkRes(BkResponse::LeavingRoomFailed(id)),
                },
            ),
            leaving_room_task: None,

            disconnection_callback: link.send_back(
                move |response: Result<(), Error>| match response {
                    Ok(_) => Msg::BkRes(BkResponse::Disconnected(id)),
                    Err(_) => Msg::BkRes(BkResponse::DisconnectionFailed(id)),
                },
            ),
            disconnection_task: None,

            session: session.clone(),
            backend: CSBackend::with_session(session),
            events_dag: None,
        }
    }
}

// This contains every informations needed for the observation of a room from a given HS by using
// the PostgreSQL backend.
pub struct PgView {
    id: ViewIndex,

    deepest_callback: Callback<Result<EventsResponse, Error>>,
    deepest_task: Option<FetchTask>,

    ancestors_callback: Callback<Result<EventsResponse, Error>>,
    ancestors_task: Option<FetchTask>,

    stop_callback: Callback<Result<(), Error>>,
    stop_task: Option<FetchTask>,

    descendants_callback: Callback<Result<EventsResponse, Error>>,
    descendants_task: Option<FetchTask>,
    descendants_timeout_task: Option<TimeoutTask>,

    state_callback: Callback<Result<EventsResponse, Error>>,
    state_task: Option<FetchTask>,

    session: Arc<RwLock<PgSession>>,
    backend: PostgresBackend,
    events_dag: Option<Arc<RwLock<RoomEvents>>>,
}

impl PgView {
    pub fn new(id: ViewIndex, link: &mut ComponentLink<Model>) -> PgView {
        let session = Arc::new(RwLock::new(PgSession::empty()));

        PgView {
            id,

            deepest_callback: link.send_back(move |response: Result<EventsResponse, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::DeepestEvents(id, res)),
                    Err(_) => Msg::BkRes(BkResponse::DeepestRqFailed(id)),
                }
            }),
            deepest_task: None,

            ancestors_callback: link.send_back(move |response: Result<EventsResponse, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::Ancestors(id, res)),
                    Err(_) => Msg::BkRes(BkResponse::AncestorsRqFailed(id)),
                }
            }),
            ancestors_task: None,

            descendants_callback: link.send_back(move |response: Result<EventsResponse, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::Descendants(id, res)),
                    Err(_) => Msg::BkRes(BkResponse::DescendantsRqFailed(id)),
                }
            }),
            descendants_task: None,
            descendants_timeout_task: None,

            state_callback: link.send_back(move |response: Result<EventsResponse, Error>| {
                match response {
                    Ok(res) => Msg::BkRes(BkResponse::State(id, res)),
                    Err(_) => Msg::BkRes(BkResponse::StateRqFailed(id)),
                }
            }),
            state_task: None,

            stop_callback: link.send_back(move |response: Result<(), Error>| match response {
                Ok(_) => Msg::BkRes(BkResponse::Disconnected(id)),
                Err(_) => Msg::BkRes(BkResponse::DisconnectionFailed(id)),
            }),
            stop_task: None,

            session: session.clone(),
            backend: PostgresBackend::with_session(session),
            events_dag: None,
        }
    }
}

// This defines which backend is used by the application for the retrieval of the events DAG.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum BackendChoice {
    CS,
    Postgres,
}

// This defines which fields of the event body will be displayed in the nodes of the displayed DAG.
struct FieldsChoice {
    sender: bool,
    origin: bool,
    origin_server_ts: bool,
    etype: bool,
    state_key: bool,
    prev_events: bool,
    depth: bool,
    redacts: bool,
    event_id: bool,

    fields: HashSet<Field>,
}

pub enum Msg {
    UI(UIEvent),
    UICmd(UICommand),
    BkCmd(BkCommand),
    BkRes(BkResponse),
}

/// These messages notifies the application of changes in the data modifiable via the UI.
pub enum UIEvent {
    ChooseCSBackend,
    ChoosePostgresBackend,
    ViewChoice(ViewIndex),
    AddView,
    ServerName(html::ChangeData),
    RoomId(html::ChangeData),

    Username(html::ChangeData),
    Password(html::ChangeData),

    ToggleSender,
    ToggleOrigin,
    ToggleOriginServerTS,
    ToggleType,
    ToggleStateKey,
    TogglePrevEvents,
    ToggleDepth,
    ToggleRedacts,
    ToggleEventID,
}

pub enum UICommand {
    DisplayEventBody,
}

/// These messages are used by the frontend to send commands to the backend.
pub enum BkCommand {
    Connect(ViewIndex),
    ListRooms(ViewIndex),
    JoinRoom(ViewIndex),
    Sync(ViewIndex),
    MoreMsg,
    FetchState,
    LeaveRoom(ViewIndex),
    Disconnect(ViewIndex),
}

/// These messages are responses from the backend to the frontend.
pub enum BkResponse {
    Connected(ViewIndex, ConnectionResponse),
    RoomsList(ViewIndex, JoinedRooms),
    RoomJoined(ViewIndex),
    Synced(ViewIndex, SyncResponse),
    MsgGot(ViewIndex, MessagesResponse),
    StateFetched(ViewIndex, ContextResponse),
    RoomLeft(ViewIndex),
    Disconnected(ViewIndex),

    ConnectionFailed(ViewIndex),
    ListingRoomsFailed(ViewIndex),
    JoiningRoomFailed(ViewIndex),
    SyncFailed(ViewIndex),
    MoreMsgFailed(ViewIndex),
    FetchStateFailed(ViewIndex),
    LeavingRoomFailed(ViewIndex),
    DisconnectionFailed(ViewIndex),

    DeepestEvents(ViewIndex, EventsResponse),
    Ancestors(ViewIndex, EventsResponse),
    Descendants(ViewIndex, EventsResponse),
    State(ViewIndex, EventsResponse),

    DeepestRqFailed(ViewIndex),
    AncestorsRqFailed(ViewIndex),
    DescendantsRqFailed(ViewIndex),
    StateRqFailed(ViewIndex),
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, mut link: ComponentLink<Self>) -> Self {
        let bk_type = Arc::new(RwLock::new(BackendChoice::CS));
        let default_view = vec![View::CS(CSView::new(0, &mut link))];

        let default_fields_choice = FieldsChoice {
            sender: false,
            origin: false,
            origin_server_ts: false,
            etype: false,
            state_key: false,
            prev_events: false,
            depth: false,
            redacts: false,
            event_id: true,

            fields: [Field::EventID].iter().cloned().collect(),
        };

        Model {
            console: ConsoleService::new(),
            timeout: TimeoutService::new(),
            vis: VisJsService::new(bk_type.clone()),

            link,

            bk_type: bk_type,
            view_idx: 0,
            views: default_view,
            event_body: None,
            room_state: None,
            fields_choice: default_fields_choice,
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
            UIEvent::ChooseCSBackend => {
                *self.bk_type.write().unwrap() = BackendChoice::CS;

                let mut new_views: Vec<CSView> = (0..self.views.len())
                    .map(|id| CSView::new(id, &mut self.link))
                    .collect();

                for (old_view, new_view) in self.views.iter().zip(new_views.iter_mut()) {
                    if let View::Postgres(old_view) = old_view {
                        let pg_session = old_view.session.read().unwrap();
                        let mut new_session = new_view.session.write().unwrap();

                        new_session.server_name = pg_session.server_name.clone();
                        new_session.room_id = pg_session.room_id.clone();
                    }
                }

                let new_views = new_views.into_iter().map(|view| View::CS(view)).collect();

                self.views = new_views;
            }
            UIEvent::ChoosePostgresBackend => {
                *self.bk_type.write().unwrap() = BackendChoice::Postgres;

                let mut new_views: Vec<PgView> = (0..self.views.len())
                    .map(|id| PgView::new(id, &mut self.link))
                    .collect();

                for (old_view, new_view) in self.views.iter().zip(new_views.iter_mut()) {
                    if let View::CS(old_view) = old_view {
                        let cs_session = old_view.session.read().unwrap();
                        let mut new_session = new_view.session.write().unwrap();

                        new_session.server_name = cs_session.server_name.clone();
                        new_session.room_id = cs_session.room_id.clone();
                    }
                }

                let new_views = new_views
                    .into_iter()
                    .map(|view| View::Postgres(view))
                    .collect();

                self.views = new_views;
            }
            UIEvent::ViewChoice(vc) => {
                let input: web::html_element::InputElement = web::document()
                    .query_selector("#server-name-input")
                    .expect("Couldn't get document element")
                    .expect("Couldn't get document element")
                    .try_into()
                    .unwrap();
                input.set_raw_value("");

                let input: web::html_element::InputElement = web::document()
                    .query_selector("#room-id-input")
                    .expect("Couldn't get document element")
                    .expect("Couldn't get document element")
                    .try_into()
                    .unwrap();
                input.set_raw_value("");

                if *self.bk_type.read().unwrap() == BackendChoice::CS {
                    let input: web::html_element::InputElement = web::document()
                        .query_selector("#username-input")
                        .expect("Couldn't get document element")
                        .expect("Couldn't get document element")
                        .try_into()
                        .unwrap();
                    input.set_raw_value("");

                    let input: web::html_element::InputElement = web::document()
                        .query_selector("#password-input")
                        .expect("Couldn't get document element")
                        .expect("Couldn't get document element")
                        .try_into()
                        .unwrap();
                    input.set_raw_value("");
                }

                self.view_idx = vc;
            }
            UIEvent::AddView => {
                let view = match *self.bk_type.read().unwrap() {
                    BackendChoice::CS => View::CS(CSView::new(self.views.len(), &mut self.link)),
                    BackendChoice::Postgres => {
                        View::Postgres(PgView::new(self.views.len(), &mut self.link))
                    }
                };

                self.views.push(view);

                self.console.log("View added");
            }
            UIEvent::ServerName(sn) => {
                if let html::ChangeData::Value(sn) = sn {
                    match &self.views[self.view_idx] {
                        View::CS(view) => view.session.write().unwrap().server_name = sn,
                        View::Postgres(view) => view.session.write().unwrap().server_name = sn,
                    }
                }
            }
            UIEvent::RoomId(ri) => {
                if let html::ChangeData::Value(ri) = ri {
                    for view in &self.views {
                        match view {
                            View::CS(view) => {
                                view.session.write().unwrap().room_id = ri.clone();
                            }
                            View::Postgres(view) => {
                                view.session.write().unwrap().room_id = ri.clone();
                            }
                        }
                    }
                }
            }
            UIEvent::Username(u) => {
                if let html::ChangeData::Value(u) = u {
                    if let View::CS(view) = &mut self.views[self.view_idx] {
                        view.session.write().unwrap().username = u;
                    }
                }
            }
            UIEvent::Password(p) => {
                if let html::ChangeData::Value(p) = p {
                    if let View::CS(view) = &mut self.views[self.view_idx] {
                        view.session.write().unwrap().password = p;
                    }
                }
            }
            UIEvent::ToggleSender => {
                let fc = &mut self.fields_choice;

                fc.sender = !fc.sender;

                if fc.sender {
                    fc.fields.insert(Field::Sender);
                } else {
                    fc.fields.remove(&Field::Sender);
                }

                for view in &self.views {
                    if let Some(events_dag) = view.get_events_dag() {
                        let mut events_dag = events_dag.write().unwrap();

                        events_dag.change_fields(&fc.fields);
                    }

                    if self.vis.is_active() {
                        if let Some(events_dag) = view.get_events_dag() {
                            self.vis.update_labels(events_dag.clone(), view.get_id());
                        }
                    }
                }
            }
            UIEvent::ToggleOrigin => {
                let fc = &mut self.fields_choice;

                fc.origin = !fc.origin;

                if fc.origin {
                    fc.fields.insert(Field::Origin);
                } else {
                    fc.fields.remove(&Field::Origin);
                }

                for view in &self.views {
                    if let Some(events_dag) = view.get_events_dag() {
                        let mut events_dag = events_dag.write().unwrap();

                        events_dag.change_fields(&fc.fields);
                    }

                    if self.vis.is_active() {
                        if let Some(events_dag) = view.get_events_dag() {
                            self.vis.update_labels(events_dag.clone(), view.get_id());
                        }
                    }
                }
            }
            UIEvent::ToggleOriginServerTS => {
                let fc = &mut self.fields_choice;

                fc.origin_server_ts = !fc.origin_server_ts;

                if fc.origin_server_ts {
                    fc.fields.insert(Field::OriginServerTS);
                } else {
                    fc.fields.remove(&Field::OriginServerTS);
                }

                for view in &self.views {
                    if let Some(events_dag) = view.get_events_dag() {
                        let mut events_dag = events_dag.write().unwrap();

                        events_dag.change_fields(&fc.fields);
                    }

                    if self.vis.is_active() {
                        if let Some(events_dag) = view.get_events_dag() {
                            self.vis.update_labels(events_dag.clone(), view.get_id());
                        }
                    }
                }
            }
            UIEvent::ToggleType => {
                let fc = &mut self.fields_choice;

                fc.etype = !fc.etype;

                if fc.etype {
                    fc.fields.insert(Field::Type);
                } else {
                    fc.fields.remove(&Field::Type);
                }

                for view in &self.views {
                    if let Some(events_dag) = view.get_events_dag() {
                        let mut events_dag = events_dag.write().unwrap();

                        events_dag.change_fields(&fc.fields);
                    }

                    if self.vis.is_active() {
                        if let Some(events_dag) = view.get_events_dag() {
                            self.vis.update_labels(events_dag.clone(), view.get_id());
                        }
                    }
                }
            }
            UIEvent::ToggleStateKey => {
                let fc = &mut self.fields_choice;

                fc.state_key = !fc.state_key;

                if fc.state_key {
                    fc.fields.insert(Field::StateKey);
                } else {
                    fc.fields.remove(&Field::StateKey);
                }

                for view in &self.views {
                    if let Some(events_dag) = view.get_events_dag() {
                        let mut events_dag = events_dag.write().unwrap();

                        events_dag.change_fields(&fc.fields);
                    }

                    if self.vis.is_active() {
                        if let Some(events_dag) = view.get_events_dag() {
                            self.vis.update_labels(events_dag.clone(), view.get_id());
                        }
                    }
                }
            }
            UIEvent::TogglePrevEvents => {
                let fc = &mut self.fields_choice;

                fc.prev_events = !fc.prev_events;

                if fc.prev_events {
                    fc.fields.insert(Field::PrevEvents);
                } else {
                    fc.fields.remove(&Field::PrevEvents);
                }

                for view in &self.views {
                    if let Some(events_dag) = view.get_events_dag() {
                        let mut events_dag = events_dag.write().unwrap();

                        events_dag.change_fields(&fc.fields);
                    }

                    if self.vis.is_active() {
                        if let Some(events_dag) = view.get_events_dag() {
                            self.vis.update_labels(events_dag.clone(), view.get_id());
                        }
                    }
                }
            }
            UIEvent::ToggleDepth => {
                let fc = &mut self.fields_choice;

                fc.depth = !fc.depth;

                if fc.depth {
                    fc.fields.insert(Field::Depth);
                } else {
                    fc.fields.remove(&Field::Depth);
                }

                for view in &self.views {
                    if let Some(events_dag) = view.get_events_dag() {
                        let mut events_dag = events_dag.write().unwrap();

                        events_dag.change_fields(&fc.fields);
                    }

                    if self.vis.is_active() {
                        if let Some(events_dag) = view.get_events_dag() {
                            self.vis.update_labels(events_dag.clone(), view.get_id());
                        }
                    }
                }
            }
            UIEvent::ToggleRedacts => {
                let fc = &mut self.fields_choice;

                fc.redacts = !fc.redacts;

                if fc.redacts {
                    fc.fields.insert(Field::Redacts);
                } else {
                    fc.fields.remove(&Field::Redacts);
                }

                for view in &self.views {
                    if let Some(events_dag) = view.get_events_dag() {
                        let mut events_dag = events_dag.write().unwrap();

                        events_dag.change_fields(&fc.fields);
                    }

                    if self.vis.is_active() {
                        if let Some(events_dag) = view.get_events_dag() {
                            self.vis.update_labels(events_dag.clone(), view.get_id());
                        }
                    }
                }
            }
            UIEvent::ToggleEventID => {
                let fc = &mut self.fields_choice;

                fc.event_id = !fc.event_id;

                if fc.event_id {
                    fc.fields.insert(Field::EventID);
                } else {
                    fc.fields.remove(&Field::EventID);
                }

                for view in &self.views {
                    if let Some(events_dag) = view.get_events_dag() {
                        let mut events_dag = events_dag.write().unwrap();

                        events_dag.change_fields(&fc.fields);
                    }

                    if self.vis.is_active() {
                        if let Some(events_dag) = view.get_events_dag() {
                            self.vis.update_labels(events_dag.clone(), view.get_id());
                        }
                    }
                }
            }
        }
    }

    fn process_ui_command(&mut self, cmd: UICommand) {
        match cmd {
            UICommand::DisplayEventBody => {
                let view_selection_input: web::html_element::InputElement = web::document()
                    .query_selector("#targeted-view")
                    .expect("Couldn't get document element")
                    .expect("Couldn't get document element")
                    .try_into()
                    .unwrap();
                let view_id: ViewIndex = view_selection_input
                    .raw_value()
                    .parse()
                    .expect("Failed to parse view_id");

                let event_id_input: web::html_element::InputElement = web::document()
                    .query_selector("#selected-event")
                    .expect("Couldn't get document element")
                    .expect("Couldn't get document element")
                    .try_into()
                    .unwrap();
                let event_id = event_id_input.raw_value();

                if let Some(dag) = self.views[view_id].get_events_dag() {
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
            BkCommand::Connect(_) => "Connecting...",
            BkCommand::ListRooms(_) => "Listing joined rooms...",
            BkCommand::JoinRoom(_) => "Joining the room...",
            BkCommand::Sync(_) => "Syncing...",
            BkCommand::MoreMsg => "Retrieving previous messages...",
            BkCommand::FetchState => "Fetching the state of the room...",
            BkCommand::LeaveRoom(_) => "Leaving the room...",
            BkCommand::Disconnect(_) => "Disconnecting...",
        };

        self.console.log(console_msg);

        // Order the backend to make requests to the homeserver according to the command received
        match cmd {
            BkCommand::Connect(view_id) => match &mut self.views[view_id] {
                View::CS(view) => match view.session.read().unwrap().access_token {
                    None => match view.connection_task {
                        None => {
                            view.connection_task =
                                Some(view.backend.connect(view.connection_callback.clone()))
                        }
                        Some(_) => self.console.log("Already connecting"),
                    },
                    Some(_) => self.console.log("You are already connected"),
                },
                View::Postgres(view) => match view.events_dag {
                    None => match view.deepest_task {
                        None => {
                            view.deepest_task =
                                Some(view.backend.deepest(view.deepest_callback.clone()))
                        }
                        Some(_) => self.console.log("Already fetching deepest events"),
                    },
                    Some(_) => self.console.log("Deepest events already fetched"),
                },
            },
            BkCommand::ListRooms(view_id) => {
                if let View::CS(view) = &mut self.views[view_id] {
                    view.listing_rooms_task =
                        Some(view.backend.list_rooms(view.listing_rooms_callback.clone()))
                }
            }
            BkCommand::JoinRoom(view_id) => {
                if let View::CS(view) = &mut self.views[view_id] {
                    view.joining_room_task =
                        Some(view.backend.join_room(view.joining_room_callback.clone()))
                }
            }
            BkCommand::Sync(view_id) => match &mut self.views[view_id] {
                View::CS(view) => {
                    let next_batch_token = view.session.read().unwrap().next_batch_token.clone();

                    view.sync_task = Some(
                        view.backend
                            .sync(view.sync_callback.clone(), next_batch_token),
                    )
                }
                View::Postgres(view) => {
                    if let Some(dag) = &view.events_dag {
                        let from = &dag.read().unwrap().latest_events;

                        view.descendants_task = Some(
                            view.backend
                                .descendants(view.descendants_callback.clone(), from),
                        );
                    }
                }
            },
            BkCommand::MoreMsg => {
                let view_selection_input: web::html_element::InputElement = web::document()
                    .query_selector("#targeted-view")
                    .expect("Couldn't get document element")
                    .expect("Couldn't get document element")
                    .try_into()
                    .unwrap();
                let view_id: ViewIndex = view_selection_input
                    .raw_value()
                    .parse()
                    .expect("Failed to parse view_id");

                match &mut self.views[view_id] {
                    View::CS(view) => match view.more_msg_task {
                        None => {
                            view.more_msg_task = Some(
                                view.backend
                                    .get_prev_messages(view.more_msg_callback.clone()),
                            );
                        }
                        Some(_) => self.console.log("Already fetching previous messages"),
                    },
                    View::Postgres(view) => match view.ancestors_task {
                        None => match &view.events_dag {
                            Some(_) => {
                                let input: web::html_element::InputElement = web::document()
                                    .query_selector("#ancestors-id")
                                    .expect("Couldn't get document element")
                                    .expect("Couldn't get document element")
                                    .try_into()
                                    .unwrap();
                                let from = vec![input.raw_value()];

                                view.ancestors_task = Some(
                                    view.backend
                                        .ancestors(view.ancestors_callback.clone(), &from),
                                );
                            }
                            None => self.console.log("There was no DAG"),
                        },
                        Some(_) => self.console.log("Already fetching ancestors"),
                    },
                }
            }
            BkCommand::FetchState => {
                let view_selection_input: web::html_element::InputElement = web::document()
                    .query_selector("#targeted-view")
                    .expect("Couldn't get document element")
                    .expect("Couldn't get document element")
                    .try_into()
                    .unwrap();
                let view_id: ViewIndex = view_selection_input
                    .raw_value()
                    .parse()
                    .expect("Failed to parse view_id");

                let event_id_input: web::html_element::InputElement = web::document()
                    .query_selector("#selected-event")
                    .expect("Couldn't get document element")
                    .expect("Couldn't get document element")
                    .try_into()
                    .unwrap();
                let event_id = event_id_input.raw_value();

                match &mut self.views[view_id] {
                    View::CS(view) => match view.state_task {
                        None => {
                            view.state_task = Some(
                                view.backend
                                    .room_state(view.state_callback.clone(), &event_id),
                            )
                        }
                        Some(_) => self.console.log("Already fetching the state of the room"),
                    },
                    View::Postgres(view) => match view.state_task {
                        None => {
                            view.state_task =
                                Some(view.backend.state(view.state_callback.clone(), &event_id))
                        }
                        Some(_) => self.console.log("Already fetching the state of the room"),
                    },
                }
            }
            BkCommand::LeaveRoom(view_id) => {
                if let View::CS(view) = &mut self.views[view_id] {
                    match view.leaving_room_task {
                        None => {
                            view.leaving_room_task =
                                Some(view.backend.leave_room(view.leaving_room_callback.clone()))
                        }
                        Some(_) => self.console.log("Already leaving the room"),
                    }
                }
            }
            BkCommand::Disconnect(view_id) => match &mut self.views[view_id] {
                View::CS(view) => match view.session.read().unwrap().access_token {
                    None => {
                        self.console.log("You were not connected");
                    }
                    Some(_) => match view.disconnection_task {
                        None => {
                            view.disconnection_task =
                                Some(view.backend.disconnect(view.disconnection_callback.clone()))
                        }
                        Some(_) => self.console.log("Already disconnecting"),
                    },
                },
                View::Postgres(view) => {
                    if view.session.read().unwrap().connected {
                        self.console.log("Stopping the backend");

                        match view.stop_task {
                            None => {
                                view.stop_task = Some(view.backend.stop(view.stop_callback.clone()))
                            }
                            Some(_) => self.console.log("Already stopping the backend"),
                        }
                    } else {
                        self.console.log("You were not connected");
                    }
                }
            },
        }
    }

    fn process_bk_response(&mut self, res: BkResponse) {
        match res {
            BkResponse::Connected(view_id, res) => {
                if let View::CS(view) = &mut self.views[view_id] {
                    view.connection_task = None;

                    let mut session = view.session.write().unwrap();

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
                        .send_back(move |_: ()| Msg::BkCmd(BkCommand::ListRooms(view_id)))
                        .emit(());
                }
            }
            BkResponse::RoomsList(view_id, res) => {
                self.console.log("Looking up in joined rooms");

                if let View::CS(view) = &mut self.views[view_id] {
                    view.listing_rooms_task = None;

                    if res
                        .joined_rooms
                        .contains(&view.session.read().unwrap().room_id)
                    {
                        // If the user is already in the room to observe, make the initial sync
                        self.link
                            .send_back(move |_: ()| Msg::BkCmd(BkCommand::Sync(view_id)))
                            .emit(());
                    } else {
                        // Join the room if the user is not already in it
                        self.link
                            .send_back(move |_: ()| Msg::BkCmd(BkCommand::JoinRoom(view_id)))
                            .emit(());
                    }
                }
            }
            BkResponse::RoomJoined(view_id) => {
                self.console.log("Room joined!");

                if let View::CS(view) = &mut self.views[view_id] {
                    view.joining_room_task = None;

                    // Make the initial sync as soon as the user has joined the room
                    self.link
                        .send_back(move |_: ()| Msg::BkCmd(BkCommand::Sync(view_id)))
                        .emit(());
                }
            }
            BkResponse::Synced(view_id, res) => {
                if let View::CS(view) = &mut self.views[view_id] {
                    view.sync_task = None;

                    let mut session = view.session.write().unwrap();
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
                                &self.fields_choice.fields,
                                res,
                            ) {
                                view.events_dag = Some(Arc::new(RwLock::new(dag)));
                            }

                            match view.events_dag.clone() {
                                Some(dag) => {
                                    // Display the DAG with VisJs if it has been successfully built
                                    if !self.vis.is_active() {
                                        self.vis.init(
                                            "#dag-vis",
                                            "#targeted-view",
                                            "#more-ev-target",
                                            "#selected-event",
                                            "#display-body-target",
                                            "#ancestors-id",
                                            "#ancestors-target",
                                        );
                                    }

                                    self.vis.add_dag(dag, view_id);
                                }
                                None => self.console.log("Failed to build the DAG"),
                            }
                        }
                        Some(_) => match view.events_dag.clone() {
                            // Add new events to the DAG
                            Some(dag) => {
                                if let Some(room) = res.rooms.join.get(&session.room_id) {
                                    dag.write()
                                        .unwrap()
                                        .add_events(room.timeline.events.clone());
                                    self.vis.update_dag(dag, view_id);
                                }
                            }
                            None => self.console.log("There is no DAG"),
                        },
                    }

                    session.next_batch_token = Some(next_batch_token);

                    // Request for futur new events
                    self.link
                        .send_back(move |_: ()| Msg::BkCmd(BkCommand::Sync(view_id)))
                        .emit(());
                }
            }
            BkResponse::MsgGot(view_id, res) => {
                if let View::CS(view) = &mut self.views[view_id] {
                    view.more_msg_task = None;

                    // Save the prev batch token for the next `/messages` request
                    view.session.write().unwrap().prev_batch_token = Some(res.end);

                    match view.events_dag.clone() {
                        // Add earlier event to the DAG and display them
                        Some(dag) => {
                            dag.write().unwrap().add_events(res.chunk);

                            self.vis.update_dag(dag, view_id);
                        }
                        None => self.console.log("There was no DAG"),
                    }
                }
            }
            BkResponse::StateFetched(view_id, res) => {
                if let View::CS(view) = &mut self.views[view_id] {
                    view.state_task = None;

                    let event_bodies = res.state.clone();

                    let object = json!({ "events": event_bodies });

                    self.room_state = match serde_json::to_string_pretty(&object) {
                        Ok(state) => Some(state),
                        Err(_) => None,
                    }
                }
            }
            BkResponse::RoomLeft(view_id) => {
                if let View::CS(view) = &mut self.views[view_id] {
                    view.leaving_room_task = None;

                    self.console.log("Room left!");

                    // Disconnect as soon as we leave the room
                    self.link
                        .send_back(move |_: ()| Msg::BkCmd(BkCommand::Disconnect(view_id)))
                        .emit(());
                }
            }
            BkResponse::Disconnected(view_id) => {
                match &mut self.views[view_id] {
                    View::CS(view) => {
                        self.console.log("Disconnected");

                        view.sync_task = None; // If a `/sync` request was in progress, cancel it
                        view.disconnection_task = None;

                        let mut session = view.session.write().unwrap();

                        // Erase the current session data so they won't be erroneously used if the user
                        // logs in again
                        session.access_token = None;
                        session.device_id = None;
                        session.filter_id = None;
                        session.next_batch_token = None;
                        session.prev_batch_token = None;

                        view.events_dag = None;
                        self.vis.remove_dag(view_id);

                        self.event_body = None;
                        self.room_state = None;
                    }
                    View::Postgres(view) => {
                        self.console.log("Backend stopped");

                        view.stop_task = None;

                        let mut session = view.session.write().unwrap();

                        session.connected = false;
                        view.descendants_timeout_task = None;
                        view.events_dag = None;
                        self.vis.remove_dag(view_id);

                        self.event_body = None;
                        self.room_state = None;
                    }
                }
            }

            BkResponse::ConnectionFailed(view_id) => {
                self.console.log("Connection failed");

                if let View::CS(view) = &mut self.views[view_id] {
                    view.connection_task = None;
                }
            }
            BkResponse::ListingRoomsFailed(view_id) => {
                self.console.log("Failed to get the list of joined rooms");

                if let View::CS(view) = &mut self.views[view_id] {
                    view.listing_rooms_task = None;
                }
            }
            BkResponse::JoiningRoomFailed(view_id) => {
                self.console.log("Failed to join the room");

                if let View::CS(view) = &mut self.views[view_id] {
                    view.joining_room_task = None;
                }
            }
            BkResponse::SyncFailed(view_id) => {
                self.console.log("Could not sync");

                if let View::CS(view) = &mut self.views[view_id] {
                    view.sync_task = None;
                }
            }
            BkResponse::MoreMsgFailed(view_id) => {
                self.console.log("Could not retrieve previous messages");

                if let View::CS(view) = &mut self.views[view_id] {
                    view.more_msg_task = None;
                }
            }
            BkResponse::FetchStateFailed(view_id) => {
                self.console.log("Could not fetch the state of the room");

                if let View::CS(view) = &mut self.views[view_id] {
                    view.more_msg_task = None;
                }
            }
            BkResponse::LeavingRoomFailed(view_id) => {
                self.console.log("Failed to leave the room");

                if let View::CS(view) = &mut self.views[view_id] {
                    view.leaving_room_task = None;
                }
            }
            BkResponse::DisconnectionFailed(view_id) => {
                self.console.log("Could not disconnect");

                if let View::CS(view) = &mut self.views[view_id] {
                    view.disconnection_task = None;
                }
            }

            BkResponse::DeepestEvents(view_id, res) => {
                if let View::Postgres(view) = &mut self.views[view_id] {
                    view.deepest_task = None;

                    let mut session = view.session.write().unwrap();
                    session.connected = true;

                    view.events_dag = Some(Arc::new(RwLock::new(
                        model::dag::RoomEvents::from_deepest_events(
                            &session.room_id,
                            &session.server_name,
                            &self.fields_choice.fields,
                            res,
                        ),
                    )));

                    match view.events_dag.clone() {
                        Some(dag) => {
                            if !self.vis.is_active() {
                                self.vis.init(
                                    "#dag-vis",
                                    "#targeted-view",
                                    "#more-ev-target",
                                    "#selected-event",
                                    "#display-body-target",
                                    "#ancestors-id",
                                    "#ancestors-target",
                                );
                            }

                            self.vis.add_dag(dag, view_id);
                        }
                        None => self.console.log("Failed to build the DAG"),
                    }

                    view.descendants_timeout_task = Some(
                        self.timeout.spawn(
                            std::time::Duration::new(5, 0),
                            self.link
                                .send_back(move |_: ()| Msg::BkCmd(BkCommand::Sync(view_id))),
                        ),
                    );
                }
            }
            BkResponse::Ancestors(view_id, res) => {
                if let View::Postgres(view) = &mut self.views[view_id] {
                    view.ancestors_task = None;

                    match view.events_dag.clone() {
                        // Add ancestors to the DAG and display them
                        Some(dag) => {
                            dag.write().unwrap().add_events(res.events);

                            self.vis.update_dag(dag, view_id);
                        }
                        None => self.console.log("There was no DAG"),
                    }
                }
            }
            BkResponse::Descendants(view_id, res) => {
                if let View::Postgres(view) = &mut self.views[view_id] {
                    view.descendants_task = None;

                    match view.events_dag.clone() {
                        Some(dag) => {
                            dag.write().unwrap().add_events(res.events);

                            self.vis.update_dag(dag, view_id);

                            if view.session.read().unwrap().connected {
                                view.descendants_timeout_task = Some(self.timeout.spawn(
                                    std::time::Duration::new(5, 0),
                                    self.link.send_back(move |_: ()| {
                                        Msg::BkCmd(BkCommand::Sync(view_id))
                                    }),
                                ));
                            }
                        }
                        None => self.console.log("There was no DAG"),
                    }
                }
            }
            BkResponse::State(view_id, res) => {
                if let View::Postgres(view) = &mut self.views[view_id] {
                    view.state_task = None;

                    self.room_state = match serde_json::to_string_pretty(&res) {
                        Ok(state) => Some(state),
                        Err(_) => None,
                    };
                }
            }

            BkResponse::DeepestRqFailed(view_id) => {
                self.console
                    .log("Could not retrieve the room's deepest events");

                if let View::Postgres(view) = &mut self.views[view_id] {
                    view.deepest_task = None;
                }
            }
            BkResponse::AncestorsRqFailed(view_id) => {
                self.console.log("Could not retrieve the events' ancestors");

                if let View::Postgres(view) = &mut self.views[view_id] {
                    view.ancestors_task = None;
                }
            }
            BkResponse::DescendantsRqFailed(view_id) => {
                self.console
                    .log("Could not retrieve the events' descendants");

                if let View::Postgres(view) = &mut self.views[view_id] {
                    view.descendants_task = None;
                }
            }
            BkResponse::StateRqFailed(view_id) => {
                self.console.log("Could not fetch the state of the room");

                if let View::Postgres(view) = &mut self.views[view_id] {
                    view.state_task = None;
                }
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

    fn display_room_state(&self) -> Html<Model> {
        match &self.room_state {
            Some(room_state) => {
                html! {
                    <pre><code>{ room_state }</code></pre>
                }
            }
            None => {
                html! {
                    <p>{ "No room state to show yet" }</p>
                }
            }
        }
    }

    fn display_backend_choice(&self) -> Html<Self> {
        let bk_type = *self.bk_type.read().unwrap();

        let connected = self.views.iter().any(|view| match view {
            View::CS(view) => view.session.read().unwrap().access_token.is_some(),
            View::Postgres(view) => view.session.read().unwrap().connected,
        });

        if !connected {
            html! {
                <>
                    <input type="radio", id="cs-bk", name="bk-type", value="cs-bk", checked=(bk_type == BackendChoice::CS), onclick=|_| Msg::UI(UIEvent::ChooseCSBackend),/>
                    <label for="cs-bk",>{ "CS backend" }</label>
                    <input type="radio", id="pg-bk", name="bk-type", value="pg-bk", checked=(bk_type == BackendChoice::Postgres), onclick=|_| Msg::UI(UIEvent::ChoosePostgresBackend),/>
                    <label for="pg-bk",>{ "Synapse PostgreSQL backend" }</label>
                </>
            }
        } else {
            html! {
                <></>
            }
        }
    }

    fn display_view_choice(&self) -> Html<Self> {
        let entry = |id| {
            html! {
                <option value=format!("view-{}", id), onclick=|_| Msg::UI(UIEvent::ViewChoice(id)),>{ format!("View {}", id + 1) }</option>
            }
        };

        html! {
            <>
                <select id="view-select",>
                    { for (0..self.views.len()).map(entry) }
                </select>

                <button onclick=|_| Msg::UI(UIEvent::AddView),>{ "Add a view" }</button>
            </>
        }
    }

    fn display_interaction_list(&self) -> Html<Self> {
        let view_id = self.view_idx;

        match *self.bk_type.read().unwrap() {
            BackendChoice::CS => {
                html! {
                    <ul>
                        <li>{ "Server name: " }<input type="text", id="server-name-input", onchange=|e| Msg::UI(UIEvent::ServerName(e)),/></li>

                        <li>{ "Room ID: " }<input type="text", id="room-id-input", onchange=|e| Msg::UI(UIEvent::RoomId(e)),/></li>

                        <li>{ "Username: " }<input type="text", id="username-input", onchange=|e| Msg::UI(UIEvent::Username(e)),/></li>

                        <li>{ "Password: " }<input type="password", id="password-input", onchange=|e| Msg::UI(UIEvent::Password(e)),/></li>

                        <li>
                            <button onclick=|_| Msg::BkCmd(BkCommand::Connect(view_id)),>{ "Connect" }</button>
                            <button onclick=|_| Msg::BkCmd(BkCommand::Disconnect(view_id)),>{ "Disconnect" }</button>
                            <button onclick=|_| Msg::BkCmd(BkCommand::LeaveRoom(view_id)),>{ "Leave room and disconnect" }</button>
                        </li>
                    </ul>
                }
            }
            BackendChoice::Postgres => {
                html! {
                    <ul>
                        <li>{ "Server name: " }<input type="text", id="server-name-input", onchange=|e| Msg::UI(UIEvent::ServerName(e)),/></li>

                        <li>{ "Room ID: " }<input type="text", id="room-id-input", onchange=|e| Msg::UI(UIEvent::RoomId(e)),/></li>

                        <li>
                            <button onclick=|_| Msg::BkCmd(BkCommand::Connect(view_id)),>{ "Start observation" }</button>
                            <button onclick=|_| Msg::BkCmd(BkCommand::Disconnect(view_id)),>{ "Stop observation" }</button>
                        </li>
                    </ul>
                }
            }
        }
    }
}

impl Renderable<Model> for Model {
    fn view(&self) -> Html<Self> {
        html! {
            <section class="backend-choice",>
                { self.display_backend_choice() }
            </section>

            <section class="view-choice",>
                { self.display_view_choice() }
            </section>

            { self.display_interaction_list() }

            <section class="fields-choice",>
                <p>{ "Event fields to show in the DAG:" }</p>

                <ul>
                    <li>
                        <input type="checkbox", id="sender", name="sender", checked=self.fields_choice.sender, onclick=|_| Msg::UI(UIEvent::ToggleSender),/>
                        <label for="sender",>{ "Sender" }</label>
                    </li>

                    <li>
                        <input type="checkbox", id="origin", name="origin", checked=self.fields_choice.origin, onclick=|_| Msg::UI(UIEvent::ToggleOrigin),/>
                        <label for="origin",>{ "Origin" }</label>
                    </li>

                    <li>
                        <input type="checkbox", id="origin-server-ts", name="origin-server-ts", checked=self.fields_choice.origin_server_ts, onclick=|_| Msg::UI(UIEvent::ToggleOriginServerTS),/>
                        <label for="origin-server-ts",>{ "Origin server time stamp" }</label>
                    </li>

                    <li>
                        <input type="checkbox", id="type", name="type", checked=self.fields_choice.etype, onclick=|_| Msg::UI(UIEvent::ToggleType),/>
                        <label for="type",>{ "Type" }</label>
                    </li>

                    <li>
                        <input type="checkbox", id="state-key", name="state-key", checked=self.fields_choice.state_key, onclick=|_| Msg::UI(UIEvent::ToggleStateKey),/>
                        <label for="state-key",>{ "State key" }</label>
                    </li>

                    <li>
                        <input type="checkbox", id="prev-events", name="prev-events", checked=self.fields_choice.prev_events, onclick=|_| Msg::UI(UIEvent::TogglePrevEvents),/>
                        <label for="prev-events",>{ "Previous events" }</label>
                    </li>

                    <li>
                        <input type="checkbox", id="depth", name="depth", checked=self.fields_choice.depth, onclick=|_| Msg::UI(UIEvent::ToggleDepth),/>
                        <label for="depth",>{ "Depth" }</label>
                    </li>

                    <li>
                        <input type="checkbox", id="redacts", name="redacts", checked=self.fields_choice.redacts, onclick=|_| Msg::UI(UIEvent::ToggleRedacts),/>
                        <label for="redacts",>{ "Redacts" }</label>
                    </li>

                    <li>
                        <input type="checkbox", id="event-id", name="event-id", checked=self.fields_choice.event_id, onclick=|_| Msg::UI(UIEvent::ToggleEventID),/>
                        <label for="event-id",>{ "Event ID" }</label>
                    </li>
                </ul>
            </section>

            <section class="to-hide",>
                <input type="text", id="targeted-view",/>

                <button id="more-ev-target", onclick=|_| Msg::BkCmd(BkCommand::MoreMsg),>{ "More events" }</button>
                <input type="text", id="selected-event",/>
                <button id="display-body-target", onclick=|_| Msg::UICmd(UICommand::DisplayEventBody),>{ "Display body" }</button>

                <input type="text", id="ancestors-id",/>
                <button id="ancestors-target", onclick=|_| Msg::BkCmd(BkCommand::MoreMsg),>{ "Ancestors" }</button>
            </section>

            <div class="view",>
                <section id="dag-vis",>
                </section>

                <section id="event-body",>
                { self.display_body() }
                </section>
            </div>

            <section class="state",>
                <button onclick=|_| Msg::BkCmd(BkCommand::FetchState), disabled=self.event_body.is_none(),>
                    { "Room state at the selected event" }
                </button>

                <section id="room-state",>
                { self.display_room_state() }
                </section>
            </section>
        }
    }
}
