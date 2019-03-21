use std::collections::HashMap;

use failure::{format_err, Error};
use serde_derive::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use yew::callback::Callback;
use yew::format::{Json, Nothing};
use yew::services::fetch::{FetchTask, Request, Response, Uri};

use crate::Model;

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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Rooms {
    #[serde(default)]
    leave: HashMap<String, JsonValue>,
    #[serde(default)]
    pub join: HashMap<String, JoinedRoom>,
    #[serde(default)]
    invite: HashMap<String, JsonValue>,
}

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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Timeline {
    #[serde(default)]
    pub limited: bool,
    pub prev_batch: Option<String>,
    // TODO: Implement RoomEvent
    #[serde(default)]
    pub events: Vec<JsonValue>,
}

impl Model {
    pub fn sync(&mut self, callback: Callback<Result<SyncResponse, Error>>) -> FetchTask {
        let filter = serde_json::json!({
            "event_fields": [
                "room_id",
                "sender",
                "origin",
                "origin_server_ts",
                "type",
                "prev_events",
                "depth",
                "event_id",
            ],
            "event_format": "federation",
        });

        let filter = serde_json::to_string(&filter)
            .unwrap()
            .replace("{", "%7B")
            .replace("}", "%7D")
            .replace("[", "%5B")
            .replace("]", "%5D")
            .replace("\"", "%22")
            .replace(":", "%3A")
            .replace("#", "%23");

        let uri = Uri::builder()
            .scheme("https")
            .authority(self.session.server_name.as_str())
            .path_and_query(
                format!(
                    "/_matrix/client/r0/sync?filter={}&set_presence=offline&timeout=5000",
                    filter
                )
                .as_str(),
            )
            .build()
            .expect("Failed to build URI.");

        let request = Request::get(uri)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.session.access_token.as_ref().unwrap()),
            )
            .body(Nothing)
            .expect("Failed to build request.");

        let handler = move |response: Response<Json<Result<SyncResponse, Error>>>| {
            let (meta, Json(data)) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(data)
            } else {
                callback.emit(Err(format_err!("{}: error connecting", meta.status)))
            }
        };

        self.fetch.fetch(request, handler.into())
    }
}
