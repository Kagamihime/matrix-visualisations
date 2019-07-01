use std::sync::{Arc, RwLock};

use failure::{format_err, Error};
use serde_derive::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use yew::callback::Callback;
use yew::format::{Json, Nothing};
use yew::services::fetch::{FetchService, FetchTask, Request, Response, Uri};

use super::session::Session;

pub struct PostgresBackend {
    fetch: FetchService,
    session: Arc<RwLock<Session>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventsResponse {
    pub events: Vec<JsonValue>,
}

impl PostgresBackend {
    pub fn with_session(session: Arc<RwLock<Session>>) -> Self {
        PostgresBackend {
            fetch: FetchService::new(),
            session,
        }
    }

    pub fn deepest(&mut self, callback: Callback<Result<EventsResponse, Error>>) -> FetchTask {
        let (server_name, room_id) = {
            let session = self.session.read().unwrap();

            (session.server_name.clone(), session.room_id.clone())
        };

        let uri = Uri::builder()
            .scheme("https")
            .authority(server_name.as_str())
            .path_and_query(format!("/visualisations/deepest/{}", room_id).as_str())
            .build()
            .expect("Failed to build URI.");

        self.request(callback, uri)
    }

    pub fn ancestors(
        &mut self,
        callback: Callback<Result<EventsResponse, Error>>,
        from: &Vec<String>,
    ) -> FetchTask {
        let (server_name, room_id) = {
            let session = self.session.read().unwrap();

            (session.server_name.clone(), session.room_id.clone())
        };
        let events_list = from.join(",");

        let uri = Uri::builder()
            .scheme("https")
            .authority(server_name.as_str())
            .path_and_query(
                format!(
                    "/visualisations/ancestors/{}?from={}&limit=10",
                    room_id, events_list
                )
                .as_str(),
            )
            .build()
            .expect("Failed to build URI.");

        self.request(callback, uri)
    }

    pub fn descendants(
        &mut self,
        callback: Callback<Result<EventsResponse, Error>>,
        from: &Vec<String>,
    ) -> FetchTask {
        let (server_name, room_id) = {
            let session = self.session.read().unwrap();

            (session.server_name.clone(), session.room_id.clone())
        };
        let events_list = from.join(",");

        let uri = Uri::builder()
            .scheme("https")
            .authority(server_name.as_str())
            .path_and_query(
                format!(
                    "/visualisations/descendants/{}?from={}&limit=10",
                    room_id, events_list
                )
                .as_str(),
            )
            .build()
            .expect("Failed to build URI.");

        self.request(callback, uri)
    }

    fn request(
        &mut self,
        callback: Callback<Result<EventsResponse, Error>>,
        uri: Uri,
    ) -> FetchTask {
        let request = Request::get(uri)
            .header("Content-Type", "application/json")
            .body(Nothing)
            .expect("Failed to buid request.");

        let handler = move |response: Response<Json<Result<EventsResponse, Error>>>| {
            let (meta, Json(data)) = response.into_parts();

            if meta.status.is_success() {
                callback.emit(data)
            } else {
                callback.emit(Err(format_err!("{}: error fetching events", meta.status)))
            }
        };

        self.fetch.fetch(request, handler.into())
    }
}
