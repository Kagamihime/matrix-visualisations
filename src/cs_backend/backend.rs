use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use failure::{format_err, Error};
use serde_derive::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use yew::callback::Callback;
use yew::format::{Json, Nothing};
use yew::services::fetch::{FetchService, FetchTask, Request, Response, Uri};

use super::session::Session;

/// Represents the backend used to communicate with a homeserver via the Client-Server HTTP REST
/// API.
pub struct CSBackend {
    fetch: FetchService,
    session: Arc<RwLock<Session>>,
}

/// Represents the JSON body of a `POST /_matrix/client/r0/login` request.
#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectionRequest {
    #[serde(rename = "type")]
    typo: String,
    identifier: Identifier,
    password: String,
    initial_device_display_name: String,
}

/// Represents the `identifier` field in `ConnectionRequest`.
#[derive(Debug, Deserialize, Serialize)]
pub struct Identifier {
    #[serde(rename = "type")]
    typo: String,
    user: String,
}

/// Represents the JSON body of a response to a `POST /_matrix/client/r0/login` request.
#[derive(Debug, Deserialize)]
pub struct ConnectionResponse {
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
}

/// Represents the JSON body of a response to a `GET /_matrix/client/r0/joined_rooms` request.
#[derive(Debug, Deserialize)]
pub struct JoinedRooms {
    pub joined_rooms: Vec<String>,
}

/// Represents the JSON body of a response to a `GET /_matrix/client/r0/sync` request.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SyncResponse {
    pub next_batch: String,
    #[serde(default)]
    pub rooms: Rooms,
    presence: Option<JsonValue>,
    #[serde(default)]
    account_data: JsonValue,
    to_device: Option<JsonValue>,
    device_lists: Option<JsonValue>,
    #[serde(default)]
    device_one_time_keys_count: HashMap<String, u64>,
}

/// Represents the list of rooms in `SyncResponse`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Rooms {
    #[serde(default)]
    leave: HashMap<String, JsonValue>,
    #[serde(default)]
    pub join: HashMap<String, JoinedRoom>,
    #[serde(default)]
    invite: HashMap<String, JsonValue>,
}

/// Represents the list of rooms joined by the user in `SyncResponse`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JoinedRoom {
    #[serde(default)]
    pub unread_notifications: JsonValue,
    #[serde(default)]
    pub timeline: Timeline,
    #[serde(default)]
    pub state: State,
    #[serde(default)]
    pub account_data: JsonValue,
    #[serde(default)]
    pub ephemeral: JsonValue,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct State {
    // TODO: Implement StateEvent
    #[serde(default)]
    pub events: Vec<JsonValue>,
}

/// Represents the timeline of a room in `SyncResponse`. These are the events of the DAG the
/// application must build for the observed room.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Timeline {
    #[serde(default)]
    pub limited: bool,
    pub prev_batch: Option<String>,
    // TODO: Implement RoomEvent
    #[serde(default)]
    pub events: Vec<JsonValue>,
}

/// Represents the JSON body of a response to a `GET /_matrix/client/r0/rooms/{roomId}/messages`
/// request.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MessagesResponse {
    pub start: String,
    pub end: String,
    pub chunk: Vec<JsonValue>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ContextResponse {
    pub start: String,
    pub end: String,
    pub events_before: Vec<JsonValue>,
    pub event: JsonValue,
    pub events_after: Vec<JsonValue>,
    pub state: Vec<JsonValue>,
}

impl CSBackend {
    /// Creates a new CS backend linked to the given `session`.
    pub fn with_session(session: Arc<RwLock<Session>>) -> Self {
        CSBackend {
            fetch: FetchService::new(),
            session,
        }
    }

    /// Sends a login request to the homeserver and then calls `callback` when it gets the
    /// response.
    pub fn connect(&mut self, callback: Callback<Result<ConnectionResponse, Error>>) -> FetchTask {
        let (server_name, username, password) = {
            let session = self.session.read().unwrap();

            (
                session.server_name.clone(),
                session.username.clone(),
                session.password.clone(),
            )
        };

        let body = ConnectionRequest {
            typo: String::from("m.login.password"),
            identifier: Identifier {
                typo: String::from("m.id.user"),
                user: username,
            },
            password,
            initial_device_display_name: String::from("Matrix visualisations"),
        };

        let uri = format!("https://{}/_matrix/client/r0/login", server_name);

        let request = Request::post(uri)
            .header("Content-Type", "application/json")
            .body(Json(&body))
            .expect("Failed to build request.");

        let handler = move |response: Response<Json<Result<ConnectionResponse, Error>>>| {
            let (meta, Json(data)) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(data)
            } else {
                callback.emit(Err(format_err!("{}: error connecting", meta.status)))
            }
        };

        self.fetch.fetch(request, handler.into())
    }

    /// Sends a request to the homeserver in order to get the list of the rooms currently joined
    /// by the user and then calls `callback` when it gets the response.
    pub fn list_rooms(&mut self, callback: Callback<Result<JoinedRooms, Error>>) -> FetchTask {
        let (server_name, access_token) = {
            let session = self.session.read().unwrap();

            (session.server_name.clone(), session.access_token.clone())
        };

        let uri = format!("https://{}/_matrix/client/r0/joined_rooms", server_name);

        let request = Request::get(uri)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", access_token.expect("No access token")),
            )
            .body(Nothing)
            .expect("Failed to build request.");

        let handler = move |response: Response<Json<Result<JoinedRooms, Error>>>| {
            let (meta, Json(data)) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(data)
            } else {
                callback.emit(Err(format_err!(
                    "{}: error listing joined rooms",
                    meta.status
                )))
            }
        };

        self.fetch.fetch(request, handler.into())
    }

    /// Sends a request to the homeserver to join the room to observe and then calls `callback`
    /// when it gets the response.
    pub fn join_room(&mut self, callback: Callback<Result<(), Error>>) -> FetchTask {
        let (server_name, access_token, room_id) = {
            let session = self.session.read().unwrap();

            (
                session.server_name.clone(),
                session.access_token.clone(),
                session.room_id.clone(),
            )
        };

        let uri = Uri::builder()
            .scheme("https")
            .authority(server_name.as_str())
            .path_and_query(format!("/_matrix/client/r0/rooms/{}/join", room_id).as_str())
            .build()
            .expect("Failed to build URI.");

        let request = Request::post(uri)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", access_token.unwrap()))
            .body(Nothing)
            .expect("Failed to build request.");

        let handler = move |response: Response<Nothing>| {
            let (meta, _) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(Ok(()))
            } else {
                callback.emit(Err(format_err!("{}: error joining the room", meta.status)))
            }
        };

        self.fetch.fetch(request, handler.into())
    }

    /// Sends a request to the homeserver for making the initial sync or receiving new events and
    /// then calls `callback` when it gets the response.
    pub fn sync(
        &mut self,
        callback: Callback<Result<SyncResponse, Error>>,
        next_batch_token: Option<String>,
    ) -> FetchTask {
        let (server_name, access_token) = {
            let session = self.session.read().unwrap();

            (session.server_name.clone(), session.access_token.clone())
        };

        let filter = build_filter();
        let mut query_params = format!(
            "/_matrix/client/r0/sync?filter={}&set_presence=offline&timeout=5000",
            filter
        );
        if let Some(next_batch_token) = next_batch_token {
            query_params.push_str("&since=");
            query_params.push_str(&next_batch_token);
        }

        let uri = Uri::builder()
            .scheme("https")
            .authority(server_name.as_str())
            .path_and_query(query_params.as_str())
            .build()
            .expect("Failed to build URI.");

        let request = Request::get(uri)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", access_token.unwrap()))
            .body(Nothing)
            .expect("Failed to build request.");

        let handler = move |response: Response<Json<Result<SyncResponse, Error>>>| {
            let (meta, Json(data)) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(data)
            } else {
                callback.emit(Err(format_err!("{}: error syncing", meta.status)))
            }
        };

        self.fetch.fetch(request, handler.into())
    }

    /// Sends a request to the homeserver to get earlier events from the room to observe and then
    /// calls `callback` when it gets the response.
    pub fn get_prev_messages(
        &mut self,
        callback: Callback<Result<MessagesResponse, Error>>,
    ) -> FetchTask {
        let (server_name, access_token, room_id, prev_batch_token) = {
            let session = self.session.read().unwrap();

            (
                session.server_name.clone(),
                session.access_token.clone(),
                session.room_id.clone(),
                session.prev_batch_token.clone(),
            )
        };

        let filter = build_filter();

        let uri = Uri::builder()
            .scheme("https")
            .authority(server_name.as_str())
            .path_and_query(
                format!(
                    "/_matrix/client/r0/rooms/{}/messages?from={}&dir=b&filter={}",
                    room_id,
                    prev_batch_token.clone().unwrap_or_default(),
                    filter,
                )
                .as_str(),
            )
            .build()
            .expect("Failed to build URI.");

        let request = Request::get(uri)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", access_token.unwrap()))
            .body(Nothing)
            .expect("Failed to build request.");

        let handler = move |response: Response<Json<Result<MessagesResponse, Error>>>| {
            let (meta, Json(data)) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(data)
            } else {
                callback.emit(Err(format_err!(
                    "{}: error retrieving previous messages",
                    meta.status
                )))
            }
        };

        self.fetch.fetch(request, handler.into())
    }

    pub fn room_state(
        &mut self,
        callback: Callback<Result<ContextResponse, Error>>,
        event_id: &str,
    ) -> FetchTask {
        let (server_name, access_token, room_id) = {
            let session = self.session.read().unwrap();

            (
                session.server_name.clone(),
                session.access_token.clone(),
                session.room_id.clone(),
            )
        };

        let uri = Uri::builder()
            .scheme("https")
            .authority(server_name.as_str())
            .path_and_query(
                format!(
                    "/_matrix/client/r0/rooms/{}/context/{}?limit=0",
                    room_id, event_id,
                )
                .as_str(),
            )
            .build()
            .expect("Failed to build URI.");

        let request = Request::get(uri)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", access_token.unwrap()))
            .body(Nothing)
            .expect("Failed to build request.");

        let handler = move |response: Response<Json<Result<ContextResponse, Error>>>| {
            let (meta, Json(data)) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(data)
            } else {
                callback.emit(Err(format_err!(
                    "{}: error retrieving previous messages",
                    meta.status
                )))
            }
        };

        self.fetch.fetch(request, handler.into())
    }

    /// Sends a request to the homeserver to leave the room which was observed and then calls
    /// `callback` when it gets the response.
    pub fn leave_room(&mut self, callback: Callback<Result<(), Error>>) -> FetchTask {
        let (server_name, access_token, room_id) = {
            let session = self.session.read().unwrap();

            (
                session.server_name.clone(),
                session.access_token.clone(),
                session.room_id.clone(),
            )
        };

        let uri = Uri::builder()
            .scheme("https")
            .authority(server_name.as_str())
            .path_and_query(format!("/_matrix/client/r0/rooms/{}/leave", room_id).as_str())
            .build()
            .expect("Failed to build URI.");

        let request = Request::post(uri)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", access_token.unwrap()))
            .body(Nothing)
            .expect("Failed to build request.");

        let handler = move |response: Response<Nothing>| {
            let (meta, _) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(Ok(()))
            } else {
                callback.emit(Err(format_err!("{}: error leaving the room", meta.status)))
            }
        };

        self.fetch.fetch(request, handler.into())
    }

    /// Sends a request to the homeserver to logout and then calls `callback` when it gets the
    /// response.
    pub fn disconnect(&mut self, callback: Callback<Result<(), Error>>) -> FetchTask {
        let (server_name, access_token) = {
            let session = self.session.read().unwrap();

            (session.server_name.clone(), session.access_token.clone())
        };

        let uri = format!("https://{}/_matrix/client/r0/logout", server_name);

        let request = Request::post(uri)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", access_token.expect("No access token")),
            )
            .body(Nothing)
            .expect("Failed to build request.");

        let handler = move |response: Response<Nothing>| {
            let (meta, _) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(Ok(()))
            } else {
                callback.emit(Err(format_err!("{}: error disconnecting", meta.status)))
            }
        };

        self.fetch.fetch(request, handler.into())
    }
}

// Builds a filter which allows the application to get events in the federation format with only
// the fields required to observe the room. Events in the federation format includes informations
// like the depth of the event in the DAG and the ID of the previous events, it allows the
// application to properly build the events DAG of a room.
fn build_filter() -> String {
    let filter = serde_json::json!({
        "event_fields": [
            "room_id",
            "sender",
            "origin",
            "origin_server_ts",
            "type",
            "state_key",
            "content",
            "prev_events",
            "depth",
            "auth_events",
            "redacts",
            "unsigned",
            "event_id",
            "hashes",
            "signatures",
        ],
        "event_format": "federation",
    });

    percent_encoding::utf8_percent_encode(
        &serde_json::to_string(&filter).unwrap(),
        percent_encoding::USERINFO_ENCODE_SET,
    )
    .to_string()
}
